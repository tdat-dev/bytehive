// Turns a Muse session log (session.jsonl envelope records) into what the
// chat view shows: your messages, the agent's replies, and the steps it took
// in between, each with its result. Pure: feed it lines, read `items`. No DOM.

import {
  baseName, fileLedger, lineDiff, newMeta,
  type AskedQuestion, type Chat, type ChatItem, type StepItem, type Todo,
} from "./chatmodel";

type Input = Record<string, unknown>;
const str = (v: unknown): string => (typeof v === "string" ? v : "");
const num = (v: unknown): number => (typeof v === "number" && Number.isFinite(v) ? v : 0);

const MAX_OUTPUT = 6000;
const MAX_DIFF_LINES = 400;

function firstLine(s: string, max = 120): string {
  const line = s.trim().split(/\r?\n/)[0] ?? "";
  return line.length > max ? line.slice(0, max - 1) + "…" : line;
}

function trimOutput(s: string): string {
  const t = s.replace(/\s+$/, "");
  return t.length > MAX_OUTPUT ? t.slice(0, MAX_OUTPUT) + "\n…" : t;
}

function countDiff(d: Array<{ sign: string }>): { added: number; removed: number } {
  let added = 0, removed = 0;
  for (const l of d) { if (l.sign === "+") added++; else if (l.sign === "-") removed++; }
  return { added, removed };
}

/** A request_user_input questions array as the chat shows and answers it. */
export function museQuestions(input: unknown): AskedQuestion[] {
  if (!Array.isArray(input)) return [];
  return (input as Input[]).map((q) => ({
    question: str(q.question) || str(q.header),
    header: str(q.header) || undefined,
    multi: (q.selection as Input | undefined)?.mode === "multiple",
    options: (Array.isArray(q.options) ? (q.options as Input[]) : []).map((o) => ({ label: str(o.label), description: str(o.description) || undefined })),
  }));
}

function questionIds(input: unknown): string[] {
  return Array.isArray(input) ? (input as Input[]).map((q) => str(q.id)) : [];
}

/** How a Muse tool call reads in the chat: a verb, what it touched, and extras. */
export function describeMuseTool(name: string, input: Input): Omit<StepItem, "kind" | "id" | "done" | "at"> {
  const file = str(input.path) || str(input.file_path);
  switch (name) {
    case "powershell": {
      const cmd = str(input.command);
      const said = str(input.description);
      return { tool: name, verb: "Ran", target: said || firstLine(cmd), full: cmd, code: !said };
    }
    case "monitor": {
      const cmd = str(input.command);
      return { tool: name, verb: "Watched", target: str(input.description) || firstLine(cmd), full: cmd || undefined };
    }
    case "read_file":
      return { tool: name, verb: /\.(png|jpe?g|gif|webp|bmp)$/i.test(file) ? "Looked at" : "Read", target: baseName(file), full: file, code: true };
    case "edit_file": {
      const diff = lineDiff(str(input.find), str(input.replace)).slice(0, MAX_DIFF_LINES);
      return { tool: name, verb: "Edited", target: baseName(file), full: file, code: true, diff, ...countDiff(diff) };
    }
    case "write_file": {
      const content = str(input.content);
      const lines = content ? content.split(/\r?\n/) : [];
      return { tool: name, verb: "Wrote", target: baseName(file), full: file, code: true, diff: lines.slice(0, MAX_DIFF_LINES).map((text) => ({ sign: "+" as const, text })), added: lines.length, removed: 0 };
    }
    case "web_fetch": {
      const url = str(input.url);
      let host = url;
      try { host = new URL(url).host; } catch { /* keep the url */ }
      return { tool: name, verb: "Read", target: host, full: url };
    }
    case "web_search":
      return { tool: name, verb: "Searched the web for", target: str(input.query) };
    case "read_skill":
      return { tool: name, verb: "Used the skill", target: str(input.name) };
    case "request_user_input": {
      const questions = museQuestions(input.questions);
      const first = questions[0];
      return { tool: name, verb: "Asked you", target: first ? first.header || firstLine(first.question) : "a question", questions };
    }
    case "subagent_spawn":
      return { tool: name, verb: "Asked a helper agent to", target: str(input.task_name) || firstLine(str(input.objective)) };
    case "image_generation":
      return { tool: name, verb: "Generated an image", target: firstLine(str(input.prompt), 80) };
    default: {
      const mcp = /^mcp__(\w+)__(\w+)$/.exec(name);
      if (mcp) return { tool: name, verb: "Used", target: `${mcp[1].replace(/[_-]+/g, " ")} · ${mcp[2].replace(/_/g, " ")}` };
      return { tool: name, verb: "Used", target: name };
    }
  }
}

export function createMuseChat(): Chat {
  const items: ChatItem[] = [];
  const byId = new Map<string, ChatItem>();
  const steps = new Map<string, StepItem>();
  const meta = newMeta();
  const noteFile = fileLedger(meta);
  const counted = new Set<string>(); // envelope ids whose usage is already summed
  const filed = new Set<string>(); // steps already filed under Files changed
  const qids = new Map<string, string[]>(); // tool call id → its question ids
  let n = 0;
  const id = () => `m${n++}`;

  function onUser(text: string, at: number, key: string): void {
    if (!text.trim() || byId.has(key)) return;
    const it: ChatItem = { kind: "user", id: key, text, images: 0, at };
    byId.set(key, it);
    items.push(it);
    meta.turn++;
    meta.started ??= at;
  }

  function onText(mid: string, text: string, at: number): void {
    if (!text) return;
    const old = byId.get(mid);
    if (old && old.kind === "text") { old.text = text; old.at = at; return; }
    const it: ChatItem = { kind: "text", id: mid, text, at };
    byId.set(mid, it);
    items.push(it);
  }

  function onCalls(calls: unknown, at: number): void {
    if (!Array.isArray(calls)) return;
    for (const c of calls as Input[]) {
      const cid = str(c.call_id) || str(c.id);
      if (!cid || steps.has(cid)) continue;
      const name = str(c.name);
      let input: Input = {};
      try {
        const v: unknown = JSON.parse(str(c.args));
        if (v && typeof v === "object") input = v as Input;
      } catch { /* a form-less call */ }
      const step: StepItem = { kind: "step", id: cid, done: false, at, ...describeMuseTool(name, input) };
      if (step.questions?.length) qids.set(cid, questionIds(input.questions));
      steps.set(cid, step);
      byId.set(cid, step);
      items.push(step);
    }
  }

  function onResults(results: unknown, at: number): void {
    if (!Array.isArray(results)) return;
    for (const r of results as Input[]) {
      const step = steps.get(str(r.tool_call_id));
      if (!step || step.done) continue; // a call from before the tail, or already filed
      step.output = trimOutput(str(r.text));
      step.done = true;
      if ((step.verb === "Edited" || step.verb === "Wrote") && !filed.has(step.id)) {
        filed.add(step.id);
        noteFile({ path: step.full ?? "", name: step.target, added: step.added, removed: step.removed, step: step.id, at });
      }
    }
  }

  function onRequested(ev: Input): void {
    // The tool call's own args already carry these questions; this is the
    // backstop for a call from before the tail (whose step is missing).
    const step = steps.get(str(ev.tool_call_id));
    if (!step || step.questions?.length) return;
    step.questions = museQuestions(ev.questions);
    if (step.questions.length) qids.set(step.id, questionIds(ev.questions));
  }

  function onSettled(ev: Input): void {
    const step = steps.get(str(ev.tool_call_id));
    const answers = Array.isArray(ev.answers) ? (ev.answers as Input[]) : [];
    if (!step?.questions?.length || !answers.length) return;
    const ids = qids.get(step.id) ?? [];
    step.questions.forEach((q, k) => {
      const got = answers.find((a) => str(a.id) && str(a.id) === ids[k]) ?? answers[k];
      const label = str(got?.selected_label);
      if (label) q.answer = label;
    });
  }

  function onModel(ev: Input, envId: string): void {
    if (counted.has(envId)) return;
    counted.add(envId);
    const usage = (ev.usage ?? {}) as Input;
    const model = str(ev.model);
    if (model) meta.model = model;
    meta.context = num(usage.input_tokens) || meta.context;
    meta.output += num(usage.output_tokens);
  }

  function onTodos(ev: Input): void {
    if (!Array.isArray(ev.items)) return;
    meta.todos = (ev.items as Input[]).map((t) => ({
      text: str(t.text),
      state: (["pending", "in_progress", "completed"].includes(str(t.status)) ? str(t.status) : "pending") as Todo["state"],
    }));
  }

  function onRecord(v: Input): void {
    if (!v || typeof v !== "object") return;
    const envId = str(v.id) || id();
    const at = Math.floor(num(v.recorded_at) / 1000);
    const ev = ((v.payload ?? {}) as Input).event as Input | undefined;
    if (!ev || typeof ev !== "object") return;
    switch (str(ev.kind)) {
      case "started": {
        const prompt = str(ev.prompt);
        if (prompt) onUser(prompt, at, envId);
        break;
      }
      case "inbox_item_queued": {
        const src = (ev.source ?? {}) as Input;
        if (str(src.source) !== "user_steer") break;
        const p = (ev.payload ?? {}) as Input;
        onUser(str(p.prompt) || str(ev.body), at, envId);
        const effort = str(p.reasoning_effort);
        if (effort) meta.effort = effort;
        break;
      }
      case "assistant_message_committed": {
        const text = str(ev.text);
        if (text) onText(str(ev.message_id) || envId, text, at);
        break;
      }
      case "assistant_tool_calls_committed":
        onCalls(ev.tool_calls, at);
        break;
      case "tool_result_batch_committed":
        onResults(ev.results, at);
        break;
      case "user_input_prompt_requested":
        onRequested(ev);
        break;
      case "user_input_prompt_settled":
        onSettled(ev);
        break;
      case "model_completed":
        onModel(ev, envId);
        break;
      case "todo_snapshot_updated":
        onTodos(ev);
        break;
    }
  }

  return {
    items,
    meta,
    feed(text: string): void {
      for (const line of text.split("\n")) {
        if (!line.trim()) continue;
        let v: Input;
        try { v = JSON.parse(line) as Input; } catch { continue; }
        if (!v || typeof v !== "object") continue;
        onRecord(v);
        // A retained frame wraps records as escaped strings; read those too.
        // Envelope ids dedupe anything also present as its own line.
        if (Array.isArray(v.children)) {
          for (const c of v.children as Input[]) {
            const raw = str(c?.record_json);
            if (!raw) continue;
            try {
              const inner = JSON.parse(raw) as Input;
              if (inner && typeof inner === "object") onRecord(inner);
            } catch { /* a half-kept child */ }
          }
        }
      }
    },
  };
}
