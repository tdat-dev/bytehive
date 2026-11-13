// Saved sessions: copies the machines sealed into the relay's store, read here
// with the Remote passphrase. They open while the machine is off; nothing here
// can be sent back to an agent.

import { StoreError, list, segments, vault, type Listed } from "../store/api";
import { checks, deriveKey, openJson, unb64 } from "../store/crypto";
import { toChat, type Meta, type Wire } from "../store/format";
import { forget, recall, remember } from "../store/remember";
import { chatLine } from "./chat";
import { bar, el, link, swap } from "./ui";

/** The account in a login token's unsigned body; the relay is the one that checks it. */
export function accountOf(token: string): string {
  try {
    const body = token.split(".")[0].replace(/-/g, "+").replace(/_/g, "/");
    const a = (JSON.parse(atob(body)) as { account?: unknown }).account;
    return typeof a === "string" ? a : "";
  } catch {
    return "";
  }
}

let turn = 0;

/** The page: the passphrase first when this device has no key, then the list or one session. */
export async function showSaved(root: HTMLElement, token: string, sid?: string): Promise<void> {
  const mine = ++turn;
  const account = accountOf(token);
  try {
    const key = await recall(account);
    if (mine !== turn) return;
    if (!key) return await unlock(root, token, account, () => void showSaved(root, token, sid));
    if (sid) await reader(root, token, key, sid, mine);
    else await sessions(root, token, key, mine);
  } catch (e) {
    if (mine !== turn) return;
    page(root, "Saved sessions", el("p", e instanceof StoreError ? e.message : "The saved sessions could not be read. Try again in a moment.", "log-note"));
  }
}

function page(root: HTMLElement, title: string, ...body: HTMLElement[]): void {
  const p = el("div", undefined, "saved");
  p.append(bar({ back: { href: "#/", label: "Home" } }), el("h1", title), ...body);
  swap(root, p);
}

async function unlock(root: HTMLElement, token: string, account: string, done: () => void): Promise<void> {
  const v = await vault(token);
  if (!v) {
    page(root, "Saved sessions", el("p", "Nothing is saved yet. Turn on Remote in ByteHive on your computer and set a passphrase there.", "log-note"));
    return;
  }
  const form = el("form", undefined, "unlock");
  const label = el("label", "Remote passphrase");
  const box = el("input");
  box.type = "password";
  box.autocomplete = "current-password";
  box.required = true;
  label.append(box);
  const note = el("p", "This device remembers it for 15 days.", "hint");
  const go = el("button", "Open", "btn primary");
  go.type = "submit";
  form.append(label, note, go);
  form.onsubmit = async (e) => {
    e.preventDefault();
    go.disabled = true;
    note.textContent = "Checking.";
    const key = await deriveKey(box.value, unb64(v.salt), v.rounds);
    if (!(await checks(key, v.check))) {
      go.disabled = false;
      note.textContent = "That passphrase is not the one set in ByteHive.";
      box.select();
      return;
    }
    await remember(key, account);
    done();
  };
  page(root, "Saved sessions", el("p", "Your sessions are locked with the passphrase you set in ByteHive. The server cannot read them."), form);
  box.focus();
}

const when = (ms: number) => new Date(ms).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });

async function sessions(root: HTMLElement, token: string, key: CryptoKey, mine: number): Promise<void> {
  const rows: { s: Listed; m: Meta }[] = [];
  for (const s of await list(token)) {
    const m = await openJson<Meta>(key, `${s.sid}/meta`, s.meta);
    if (m) rows.push({ s, m });
  }
  if (mine !== turn) return;
  const forgetBtn = el("button", "Forget the passphrase on this device", "btn quiet");
  forgetBtn.type = "button";
  forgetBtn.onclick = async () => { await forget(); location.hash = "#/"; };
  if (!rows.length) return page(root, "Saved sessions", el("p", "Nothing is saved yet.", "log-note"), forgetBtn);
  const byMachine = new Map<string, { s: Listed; m: Meta }[]>();
  for (const r of rows) byMachine.set(r.m.machine, [...(byMachine.get(r.m.machine) ?? []), r]);
  const parts: HTMLElement[] = [];
  for (const [machine, list] of byMachine) {
    const sec = el("section", undefined, "saved-machine");
    const ul = el("ul", undefined, "saved-list");
    for (const { s, m } of list) {
      const li = el("li");
      const a = link("", `#/saved/${s.sid}`, "saved-row");
      a.append(el("span", m.title || "Untitled session", "saved-title"), el("span", `${m.agent}, ${m.cli}, ${m.project}, ${when(m.updated)}`, "saved-sub"));
      li.append(a);
      ul.append(li);
    }
    sec.append(el("h2", machine), ul);
    parts.push(sec);
  }
  page(root, "Saved sessions", ...parts, forgetBtn);
}

async function reader(root: HTMLElement, token: string, key: CryptoKey, sid: string, mine: number): Promise<void> {
  const listed = (await list(token)).find((s) => s.sid === sid);
  const meta = listed ? await openJson<Meta>(key, `${sid}/meta`, listed.meta) : null;
  const log = el("div", undefined, "log");
  let unreadable = 0;
  for (const seg of await segments(token, sid)) {
    const items = await openJson<Wire[]>(key, `${sid}/${seg.n}`, seg.sealed);
    if (!items) { unreadable++; continue; }
    for (const w of items) {
      const c = toChat(w);
      if (c) log.append(chatLine(c));
    }
  }
  if (mine !== turn) return;
  const sub = meta ? el("p", `${meta.agent} on ${meta.machine}, ${meta.cli}, ${meta.project}. Last change ${when(meta.updated)}.`, "hint") : el("p", "This session is gone.", "log-note");
  const extra = unreadable ? [el("p", `${unreadable} part${unreadable === 1 ? "" : "s"} could not be opened.`, "log-note")] : [];
  const p = el("div", undefined, "saved");
  p.append(bar({ back: { href: "#/saved", label: "Saved sessions" } }), el("h1", meta?.title || "Saved session"), sub, ...extra, log);
  swap(root, p);
}
