/* MCP surface: nine tools over the board store. Each mutating tool is one
 * load → mutate → save cycle (nothing cached between calls, so a long agent
 * session always sees the ByteHive UI's edits). Every error thrown along the
 * way — BoardError or otherwise — is caught and returned as an isError tool
 * result; nothing here propagates to a crash. */

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";
import { loadBoard, saveBoard, LABELS, type Board } from "./board.js";
import {
  addCard,
  updateCard,
  moveCard,
  deleteCard,
  markDone,
  addList,
  renameList,
  deleteList,
} from "./ops.js";
import { changedFiles, evidenceDir } from "./git.js";
import { readFleet, queueMessage, readAgentScreen, queueSpawn } from "./fleet.js";
import { HubClient, registerBrowserTools, BROWSER_INSTRUCTIONS } from "./browser.js";
import { companyRoot, floorDir, registerCompanyTools, sendAsMember } from "./company.js";
import { agentContext } from "./context/agentcontext.js";
import { memoryDelete, memoryForInstructions, memoryGet, memorySave, memoryUpdate } from "./memory.js";
import os from "node:os";

type ToolResult = {
  content: { type: "text"; text: string }[];
  isError?: boolean;
};

const ok = (data: unknown): ToolResult => ({
  content: [{ type: "text", text: typeof data === "string" ? data : JSON.stringify(data, null, 2) }],
});

const fail = (e: unknown): ToolResult => ({
  content: [{ type: "text", text: e instanceof Error ? e.message : String(e) }],
  isError: true,
});

const labelsSchema = z
  .array(z.string())
  .optional()
  .describe(`Label colour keys: ${LABELS.join("|")}`);

const INSTRUCTIONS = `ByteHive board — how to work in this workspace.

For any non-trivial task, plan on the board BEFORE implementing:
1. Call board_get to see the current lists and cards.
2. Break the work into FEW, BIG tasks — one card per deliverable — and put the
   small concrete steps INSIDE each card as its checklist. Do NOT create one
   card per tiny step. Use card_add with list "Proposed", a short title, a
   one-line desc, and the checklist array.
3. Stop and let the user review (they move cards to "To do" to approve).
4. While working: card_move your card to "Doing" when you start it, and
   card_done with a one-line summary when it is finished.

Keep card titles stable so the board can track them. Use fleet_status to see
the other agents here and fleet_send to hand work to a specific one.

Other agents may have worked here before you, in ByteHive or not, with any CLI.
Call agent_context first to see who did what, and agent_context with who to
pick up an unfinished task instead of starting over.

This project keeps a shared memory (.bytehive/memory.json) that every agent
reads and the user edits in ByteHive: what anyone working here should know
(conventions, decisions, where things live, gotchas). Read it with memory_get
before you start. When you learn something lasting about the project, save it
with memory_save, one fact per entry; fix a fact that turned out wrong with
memory_update, and remove a stale one with memory_delete. Never save secrets,
and never save notes about one task in progress.

Write every fleet_send message in English, whatever language the user speaks to
you in — the fleet shares one working language so no agent has to translate a
hand-off. Answer the user in the language they used.

${BROWSER_INSTRUCTIONS}`;

export function createServer(dir: string): McpServer {
  // The project's memory rides in the instructions, so an agent knows it
  // before its first tool call (read once, when the agent starts).
  const server = new McpServer({ name: "bytehive", version: "0.1.0" }, { instructions: INSTRUCTIONS + memoryForInstructions(dir) });

  // ByteHive sets BYTEHIVE_AGENT=<pane name> on every terminal it spawns, so
  // board mutations can record which agent acted (done.by, claim-on-move).
  const agentName = process.env.BYTEHIVE_AGENT?.trim() || process.env.MAESTRO_AGENT?.trim() || undefined;
  // The board lives in the project (dir); card_done evidence is read where this
  // agent works, which for an isolated agent is its worktree.
  const runDir = evidenceDir(process.env, dir);
  // Set only for members of the company; everyone else keeps the flat fleet.
  const personId = process.env.BYTEHIVE_AGENT_ID?.trim() || undefined;

  /** load → mutate → save, mapping any thrown error to an isError tool result. */
  const mutate = (fn: (b: Board) => unknown): ToolResult => {
    try {
      const board = loadBoard(dir);
      const result = fn(board);
      saveBoard(dir, board);
      return ok(result ?? "ok");
    } catch (e) {
      return fail(e);
    }
  };

  server.registerTool(
    "board_get",
    {
      description:
        "Read the ByteHive kanban board for this workspace (.bytehive/board.json): all lists and their cards, including ids to use with the other tools. The Founder can pass floor to read another project.",
      inputSchema: { floor: z.string().optional().describe("Founder only: a floor id from company_status") },
    },
    async ({ floor }) => {
      try {
        if (floor && !personId) return fail("Only members of the company have floors.");
        return ok(loadBoard(floor ? floorDir(companyRoot(), personId!, floor) : dir));
      } catch (e) {
        return fail(e);
      }
    },
  );

  server.registerTool(
    "card_add",
    {
      description:
        "Add a card to a list on the ByteHive board. `list` is a list id or title (created if the title doesn't exist).",
      inputSchema: {
        list: z.string().describe("List id or title"),
        title: z.string().describe("Card title"),
        desc: z.string().optional().describe("Longer description"),
        labels: labelsSchema,
        due: z.string().optional().describe("Due date, yyyy-mm-dd"),
        checklist: z.array(z.string()).optional().describe("Checklist item texts"),
      },
    },
    async ({ list, title, desc, labels, due, checklist }) =>
      mutate((b) => addCard(b, list, { title, desc, labels, due, checklist })),
  );

  server.registerTool(
    "card_update",
    {
      description:
        "Update fields of a card. `card` is a card id or exact title. Only provided fields change; `checklist` replaces the whole checklist with un-done items.",
      inputSchema: {
        card: z.string().describe("Card id or title"),
        title: z.string().optional(),
        desc: z.string().optional(),
        labels: labelsSchema,
        due: z.string().nullable().optional().describe("yyyy-mm-dd, or null to clear"),
        checklist: z.array(z.string()).optional(),
      },
    },
    async ({ card, ...patch }) => mutate((b) => updateCard(b, card, patch)),
  );

  server.registerTool(
    "card_move",
    {
      description:
        "Move a card to another list (e.g. To do → Doing). `position` is 0-based; omitted = end of the list.",
      inputSchema: {
        card: z.string().describe("Card id or title"),
        to_list: z.string().describe("Target list id or title"),
        position: z.number().int().min(0).optional(),
      },
    },
    async ({ card, to_list, position }) =>
      mutate((b) => moveCard(b, card, to_list, position, agentName, personId)),
  );

  server.registerTool(
    "card_delete",
    {
      description: "Delete a card from the board.",
      inputSchema: { card: z.string().describe("Card id or title") },
    },
    async ({ card }) =>
      mutate((b) => {
        deleteCard(b, card);
        return "deleted";
      }),
  );

  server.registerTool(
    "card_done",
    {
      description:
        "Mark a task finished: move the card to the Done list and attach evidence (summary + files currently changed in git).",
      inputSchema: {
        card: z.string().describe("Card id or title"),
        summary: z.string().optional().describe("One line on what changed"),
      },
    },
    async ({ card, summary }) =>
      mutate((b) =>
        markDone(b, card, { repoRoot: runDir, files: changedFiles(runDir), summary, by: agentName }),
      ),
  );

  server.registerTool(
    "list_add",
    {
      description: "Add a new empty list to the board.",
      inputSchema: { title: z.string() },
    },
    async ({ title }) => mutate((b) => addList(b, title)),
  );

  server.registerTool(
    "list_rename",
    {
      description: "Rename a list.",
      inputSchema: {
        list: z.string().describe("List id or current title"),
        title: z.string().describe("New title"),
      },
    },
    async ({ list, title }) => mutate((b) => renameList(b, list, title)),
  );

  server.registerTool(
    "list_delete",
    {
      description: "Delete a list and all its cards.",
      inputSchema: { list: z.string().describe("List id or title") },
    },
    async ({ list }) =>
      mutate((b) => {
        deleteList(b, list);
        return "deleted";
      }),
  );

  server.registerTool(
    "fleet_status",
    {
      description:
        "List the other agents ByteHive is running in this workspace and their live status (needs = waiting on the user, active = working, idle, stopped). Use to coordinate — e.g. see who is free before handing off work.",
      inputSchema: {},
    },
    async () => {
      try {
        return ok(readFleet(dir));
      } catch (e) {
        return fail(e);
      }
    },
  );

  server.registerTool(
    "fleet_send",
    {
      description:
        "Send a message to another agent in this workspace (or the whole fleet). ByteHive types it into the target agent's terminal. Use to hand off a task or coordinate. Omit `to` to broadcast to every agent. Write the message in English even when the user speaks another language — it is read by an agent, not by the user.",
      inputSchema: {
        message: z
          .string()
          .describe("The text to deliver. Write it in English — the reader is another agent."),
        to: z
          .string()
          .optional()
          .describe("Target agent name (from fleet_status). Omit to broadcast to all agents."),
      },
    },
    async ({ message, to }) => {
      try {
        if (personId) return ok(sendAsMember(companyRoot(), personId, to, message, Date.now()));
        const m = queueMessage(dir, { from: agentName, to, message, now: Date.now() });
        return ok(m.to ? `sent to ${m.to}` : "broadcast to the fleet");
      } catch (e) {
        return fail(e);
      }
    },
  );

  server.registerTool(
    "agent_output",
    {
      description:
        "Read the recent on-screen text of another agent in this workspace (by name, from fleet_status). Use to check a worker's progress or whether it is stuck/waiting.",
      inputSchema: { agent: z.string().describe("Agent name (from fleet_status)") },
    },
    async ({ agent }) => {
      try {
        const screen = readAgentScreen(dir, agent);
        if (screen === null) return fail(`no agent "${agent}" — see fleet_status`);
        return ok(screen || "(no output yet)");
      } catch (e) {
        return fail(e);
      }
    },
  );

  server.registerTool(
    "agent_context",
    {
      description:
        "What other coding agents (Claude Code, Codex, OpenCode) did in this project, including ones that stopped. Call it with no arguments when you start working here, and with who to read one agent's full context before you continue or take over its work.",
      inputSchema: {
        who: z
          .string()
          .optional()
          .describe("An agent's name, a conversation [id] from the list, or a CLI (claude, codex, opencode) for its newest conversation. Omit for the list."),
      },
    },
    async ({ who }) => {
      try {
        return ok(await agentContext(dir, who, { home: os.homedir(), now: Date.now() }));
      } catch (e) {
        return fail(e);
      }
    },
  );

  server.registerTool(
    "memory_get",
    {
      description:
        "Read this project's shared memory: the facts every agent working here should know, one per line with its id. Call it before you start a task.",
      inputSchema: {},
    },
    async () => {
      try {
        return ok(memoryGet(dir));
      } catch (e) {
        return fail(e);
      }
    },
  );

  server.registerTool(
    "memory_save",
    {
      description:
        "Save one lasting fact about this project to its shared memory, for every agent and the user (who sees it in ByteHive): a convention, a decision and why, where something lives, a gotcha. One fact per call, a sentence or two. Not secrets, not notes about the task in progress.",
      inputSchema: { fact: z.string().describe("The fact, in a sentence or two") },
    },
    async ({ fact }) => {
      try {
        return ok(memorySave(dir, fact, agentName ?? "agent", Date.now()));
      } catch (e) {
        return fail(e);
      }
    },
  );

  server.registerTool(
    "memory_update",
    {
      description: "Rewrite one fact in this project's memory (by its id from memory_get), when it is wrong or out of date.",
      inputSchema: {
        id: z.string().describe("The fact's id, like m3"),
        fact: z.string().describe("The corrected fact"),
      },
    },
    async ({ id, fact }) => {
      try {
        return ok(memoryUpdate(dir, id, fact, agentName ?? "agent", Date.now()));
      } catch (e) {
        return fail(e);
      }
    },
  );

  server.registerTool(
    "memory_delete",
    {
      description: "Remove one fact from this project's memory (by its id from memory_get), when it no longer holds.",
      inputSchema: { id: z.string().describe("The fact's id, like m3") },
    },
    async ({ id }) => {
      try {
        return ok(memoryDelete(dir, id));
      } catch (e) {
        return fail(e);
      }
    },
  );

  server.registerTool(
    "agent_spawn",
    {
      description:
        "Ask ByteHive to spawn new worker agent(s) in this workspace — for a conductor that grows its own crew. `cli` is a CLI id (claude, codex, gemini, aider, …) or a raw command. Optional `task` is typed into each new agent; `count` (1–6) spawns several.",
      inputSchema: {
        cli: z.string().describe("CLI id (claude, codex, …) or a raw command"),
        task: z.string().optional().describe("Prompt to type into each new agent"),
        count: z.number().int().min(1).max(6).optional().describe("How many to spawn (default 1)"),
      },
    },
    async ({ cli, task, count }) => {
      try {
        if (personId) return fail("Members of the company do not start agents. Hiring goes through the Founder's plan.");
        const r = queueSpawn(dir, { from: agentName, cli, task, count, now: Date.now() });
        return ok(`queued ${r.count}× ${r.cli}${r.task ? " with a task" : ""}`);
      } catch (e) {
        return fail(e);
      }
    },
  );

  if (personId) registerCompanyTools(server, personId);

  // The user's real browsers, through the ByteHive hub (see browser.ts).
  registerBrowserTools(server, new HubClient(agentName ?? "Agent"), dir, agentName ?? "Agent");

  return server;
}
