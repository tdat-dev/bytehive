// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ChatItem } from "../../../src/chatmodel";
import type { ToMachine, ToWeb } from "../../../src/remote/protocol";
import type { MachineSession } from "../session";
import { PendingList, mergeItems, openChat, promptStart, questionOf, renderMarkdown } from "./chat";

const t = (id: string, text: string): ChatItem => ({ kind: "text", id, text, at: 1 });
const ids = (items: ChatItem[]) => items.map((i) => i.id);

describe("markdown from an agent", () => {
  const drawn = (md: string) => {
    const d = document.createElement("div");
    d.innerHTML = renderMarkdown(md);
    return d;
  };
  it("never turns raw html into script, handlers, pictures or foreign frames", () => {
    const d = drawn(`<img src=x onerror="alert(1)"><script>alert(2)</script><iframe src="https://x"></iframe>![p](https://x/p.png)**ok**`);
    expect(d.querySelectorAll("img, script, iframe, [onerror]")).toHaveLength(0);
    expect(d.querySelector("strong")?.textContent).toBe("ok");
    // What was typed stays readable, as text.
    expect(d.textContent).toContain("<script>alert(2)</script>");
  });
  it("opens links on their own, and not as script", () => {
    const d = drawn(`[a](https://example.com) [b](javascript:alert(1))`);
    expect(d.querySelector("a[href='https://example.com']")?.getAttribute("rel")).toBe("noopener noreferrer");
    expect(d.innerHTML).not.toContain("javascript:");
  });
});

describe("items", () => {
  it("replaces an item that moved on and appends new ones", () => {
    const got = mergeItems([t("a", "1")], [t("a", "2"), t("b", "3")], false);
    expect(got).toEqual([t("a", "2"), t("b", "3")]);
  });
  it("puts an older page in front, once", () => {
    const got = mergeItems([t("b", "2")], [t("a", "1"), t("b", "2")], true);
    expect(ids(got)).toEqual(["a", "b"]);
  });
});

describe("a question on the page", () => {
  const yes = [{ label: "Yes", keys: "1" }, { label: "No", keys: "2" }];
  it("shows the screen up to the list its buttons already show", () => {
    expect(questionOf("Bash command\n  rm x\n\nProceed?\n❯ 1. Yes\n  2. No\n\nEsc to cancel", yes)).toBe("Bash command\n  rm x\n\nProceed?");
  });
  it("keeps the last list when the text above has one that looks the same", () => {
    expect(questionOf("Plan:\n1. Build\n2. Ship\nProceed?\n1. Yes\n2. No", yes)).toBe("Plan:\n1. Build\n2. Ship\nProceed?");
  });
  it("shows the whole screen when the last numbered list isn't the list of answers", () => {
    const three = [...yes, { label: "Later", keys: "3" }];
    const screen = "Plan:\n1. Build\n2. Ship\nRun it?\n[a] Yes\n[b] No\n[c] Later";
    expect(questionOf(screen, three)).toBe(screen);
    // More entries than answers: something else stands after the "1." too.
    const longer = "Steps:\n1. Build\n2. Ship\n3. Tell the team\nRun it?";
    expect(questionOf(longer, yes)).toBe(longer);
  });
  it("shows the whole screen when there are no buttons", () => {
    expect(questionOf("odd\n1. thing", [])).toBe("odd\n1. thing");
  });
  it("starts under the last rule a CLI drew, not at older output", () => {
    const text = "old output\n────\nolder box\n╭────╮\n│ rm -rf x │\nProceed?";
    expect(text.slice(promptStart(text)!)).toBe("│ rm -rf x │\nProceed?");
  });
  it("does not take a rule just above the question for its start", () => {
    const text = "────\nBash command\n  rm x\n────\nProceed?";
    expect(text.slice(promptStart(text)!)).toBe("Bash command\n  rm x\n────\nProceed?");
  });
  it("has no start to offer when there is no rule, however long the screen", () => {
    expect(promptStart(Array.from({ length: 22 }, (_, n) => `line ${n}`).join("\n"))).toBeNull();
  });
});

describe("pending messages", () => {
  it("keeps a message unsent when the line is down, and sends it only when asked", () => {
    const sent: string[] = [];
    const p = new PendingList((_id, text) => sent.push(text));
    const id = p.add("hello", false);
    expect(sent).toEqual([]);
    expect(p.items[0].state).toBe("unsent");
    p.retry(id);
    expect(sent).toEqual(["hello"]);
    expect(p.items[0].state).toBe("waiting");
  });
  it("follows the machine's answers until the transcript has it", () => {
    const p = new PendingList(() => {});
    const id = p.add("hi", true);
    p.ack(id, "typed");
    expect(p.items[0].state).toBe("typed");
    p.ack(id, "seen");
    expect(p.items).toEqual([]);
  });
  it("offers a retry when the text stayed in the box", () => {
    const p = new PendingList(() => {});
    const id = p.add("hi", true);
    p.ack(id, "failed");
    expect(p.items[0].state).toBe("unsent");
  });
  it("offers again what was waiting when the page closed, and forgets what was typed", () => {
    const p = new PendingList(() => {});
    p.add("one", true);
    p.ack(p.add("two", true), "typed");
    p.park();
    expect(p.items.map((x) => [x.text, x.state])).toEqual([["one", "unsent"]]);
  });
});

function session() {
  const sent: ToMachine[] = [];
  const s = { open: true, send: (m: ToMachine) => sent.push(m) } as unknown as MachineSession;
  return { s, sent };
}
const hear = (msg: ToWeb) => window.dispatchEvent(new CustomEvent("remote-message", { detail: { peer: "m:1", msg } }));
const buttons = (root: HTMLElement) => [...root.querySelectorAll("button")].map((b) => b.textContent);

const closers: (() => void)[] = [];
afterEach(() => {
  closers.splice(0).forEach((c) => c());
});

describe("the chat on the page", () => {
  it("asks for the newest page and draws what comes, with an agent's text as text", () => {
    const { s, sent } = session();
    const root = document.createElement("div");
    closers.push(openChat(root, s, "m:1", "a-open", () => {}));
    expect(sent).toEqual([{ type: "chat.page", agent: "a-open", before: null, limit: 40 }]);
    hear({ type: "chat.items", agent: "a-open", older: true, partial: false, live: false, reset: false, items: [
      { kind: "user", id: "u", text: "<b>mine</b>", images: 0, at: 1 },
      t("x", `**hi** <img src=x onerror="window.__ran=1">`),
    ] });
    expect(root.querySelector(".user")?.textContent).toBe("<b>mine</b>");
    expect(root.querySelector(".user b")).toBeNull();
    expect(root.querySelector(".text strong")?.textContent).toBe("hi");
    expect(root.querySelector(".text img")).toBeNull();
    expect(buttons(root)).toContain("Show earlier");
    expect(root.querySelector(".log")?.firstElementChild?.textContent).toBe("Show earlier");
  });

  it("asks for the page before the oldest item it has", () => {
    const { s, sent } = session();
    const root = document.createElement("div");
    closers.push(openChat(root, s, "m:1", "a-earlier", () => {}));
    hear({ type: "chat.items", agent: "a-earlier", older: true, partial: false, live: false, reset: false, items: [t("b", "2")] });
    [...root.querySelectorAll("button")].find((b) => b.textContent === "Show earlier")!.click();
    expect(sent.at(-1)).toEqual({ type: "chat.page", agent: "a-earlier", before: "b", limit: 40 });
    hear({ type: "chat.items", agent: "a-earlier", older: false, partial: false, live: false, reset: false, items: [t("a", "1")] });
    expect([...root.querySelectorAll(".item")].map((e) => e.textContent?.trim())).toEqual(["1", "2"]);
    expect(buttons(root)).not.toContain("Show earlier");
  });

  it("shows a question as buttons, and a key goes out when one is pressed", () => {
    const { s, sent } = session();
    const root = document.createElement("div");
    closers.push(openChat(root, s, "m:1", "a-ask", () => {}));
    hear({ type: "prompt.ask", id: "s1", agent: "a-ask", screen: "Proceed?", options: [{ label: "Yes", keys: "1" }, { label: "No", keys: "3" }] });
    expect(buttons(root)).toContain("Yes");
    [...root.querySelectorAll("button")].find((b) => b.textContent === "No")!.click();
    expect(sent.at(-1)).toEqual({ type: "prompt.answer", agent: "a-ask", id: "s1", keys: "3" });
    expect(buttons(root)).not.toContain("No");
  });

  it("offers the terminal for a question it can't read", () => {
    const { s } = session();
    const root = document.createElement("div");
    const onTerminal = vi.fn();
    closers.push(openChat(root, s, "m:1", "a-term", onTerminal));
    hear({ type: "prompt.ask", id: "s1", agent: "a-term", screen: "odd screen", options: [] });
    [...root.querySelectorAll("button")].find((b) => b.textContent === "Open the terminal to answer")!.click();
    expect(onTerminal).toHaveBeenCalled();
  });

  it("drops a question once the agent no longer needs an answer", () => {
    const { s } = session();
    const root = document.createElement("div");
    closers.push(openChat(root, s, "m:1", "a-gone", () => {}));
    hear({ type: "prompt.ask", id: "s1", agent: "a-gone", screen: "Proceed?", options: [{ label: "Yes", keys: "1" }] });
    hear({ type: "fleet.state", projects: [], presets: [], agents: [{ id: "a-gone", name: "c", project: "p", projectId: "p1", status: "working", title: null, hasChat: true, cols: 80, rows: 24 }] });
    expect(buttons(root)).not.toContain("Yes");
  });

  it("starts over from a reset instead of merging by id, and leaves 'older' alone for a pushed message", () => {
    const { s } = session();
    const root = document.createElement("div");
    closers.push(openChat(root, s, "m:1", "a-reset", () => {}));
    hear({ type: "chat.items", agent: "a-reset", older: true, partial: true, live: false, reset: false, items: [t("c5", "old five"), t("c6", "old six")] });
    hear({ type: "chat.items", agent: "a-reset", older: false, partial: true, live: true, reset: false, items: [t("c7", "pushed")] });
    expect(buttons(root)).toContain("Show earlier");
    // Another parse: c5 now means another message.
    hear({ type: "chat.items", agent: "a-reset", older: false, partial: false, live: false, reset: true, items: [t("c5", "new five")] });
    expect([...root.querySelectorAll(".item")].map((e) => e.textContent?.trim())).toEqual(["new five"]);
    expect(buttons(root)).not.toContain("Show earlier");
    expect(buttons(root)).not.toContain("Load the whole conversation");
  });

  it("drops the buttons when the agent stops asking", () => {
    const { s } = session();
    const root = document.createElement("div");
    closers.push(openChat(root, s, "m:1", "a-clear", () => {}));
    hear({ type: "prompt.ask", agent: "a-clear", id: "s1", screen: "Proceed?", options: [{ label: "Yes", keys: "1" }] });
    expect(buttons(root)).toContain("Yes");
    hear({ type: "prompt.clear", agent: "a-other" });
    expect(buttons(root)).toContain("Yes");
    hear({ type: "prompt.clear", agent: "a-clear" });
    expect(buttons(root)).not.toContain("Yes");
  });

  it("answers the question it was last told, by its id", () => {
    const { s, sent } = session();
    const root = document.createElement("div");
    closers.push(openChat(root, s, "m:1", "a-id", () => {}));
    hear({ type: "prompt.ask", agent: "a-id", id: "first", screen: "A?", options: [{ label: "Yes", keys: "1" }] });
    hear({ type: "prompt.ask", agent: "a-id", id: "second", screen: "B?", options: [{ label: "Yes", keys: "1" }] });
    [...root.querySelectorAll("button")].find((b) => b.textContent === "Yes")!.click();
    expect(sent.at(-1)).toEqual({ type: "prompt.answer", agent: "a-id", id: "second", keys: "1" });
  });

  it("does not call a delivered message not sent after a tab switch, and hears its answer while the tab is away", () => {
    const { s, sent } = session();
    const root = document.createElement("div");
    const close = openChat(root, s, "m:1", "a-tab", () => {});
    root.querySelector("textarea")!.value = "do it";
    root.querySelector("form")!.dispatchEvent(new Event("submit", { cancelable: true }));
    const { id } = sent.at(-1) as Extract<ToMachine, { type: "chat.send" }>;
    close();

    // Back on the same line: still on its way, not "not sent".
    const back = document.createElement("div");
    const closeBack = openChat(back, s, "m:1", "a-tab", () => {});
    expect(back.querySelector(".pending")?.textContent).toBe("do it (waiting)");
    closeBack();

    // The machine answers while the Terminal tab is open.
    hear({ type: "chat.ack", id, state: "typed" });
    const last = document.createElement("div");
    closers.push(openChat(last, s, "m:1", "a-tab", () => {}));
    // Typed: the conversation asked for on opening shows it.
    expect(last.querySelector(".pending")).toBeNull();
    expect(sent.filter((m) => m.type === "chat.send")).toHaveLength(1);
  });

  it("puts a no-agent error on the message that was waiting, and offers it again", () => {
    const { s, sent } = session();
    const root = document.createElement("div");
    closers.push(openChat(root, s, "m:1", "a-noagent", () => {}));
    root.querySelector("textarea")!.value = "do it";
    root.querySelector("form")!.dispatchEvent(new Event("submit", { cancelable: true }));
    hear({ type: "error", code: "no-agent", message: "That agent isn't open on this computer." });
    expect(root.querySelector(".pending")?.textContent).toBe("do it (That agent isn't open on this computer.)Send again");
    expect(root.querySelector(".log")?.textContent).not.toContain("isn't open");
    root.querySelector<HTMLButtonElement>(".pending button")!.click();
    expect(root.querySelector(".pending")?.textContent).toBe("do it (waiting)");
    expect(sent.filter((m) => m.type === "chat.send")).toHaveLength(2);
  });

  it("sums up messages in one line while a question is open, and brings them back after the answer", () => {
    const { s } = session();
    const root = document.createElement("div");
    closers.push(openChat(root, s, "m:1", "a-summed", () => {}));
    root.querySelector("textarea")!.value = "do it";
    root.querySelector("form")!.dispatchEvent(new Event("submit", { cancelable: true }));
    hear({ type: "error", code: "no-agent", message: "That agent isn't open on this computer." });
    hear({ type: "prompt.ask", agent: "a-summed", id: "s1", screen: "Proceed?", options: [{ label: "Yes", keys: "1" }] });
    expect(root.querySelector(".pending")).toBeNull();
    expect(root.querySelector(".queue")?.textContent).toBe("1 message not sent. Shown again after you answer.");
    [...root.querySelectorAll("button")].find((b) => b.textContent === "Yes")!.click();
    expect(root.querySelector(".pending button")?.textContent).toBe("Send again");
  });

  it("hears a no-agent error while the Terminal tab is open, and offers the message again on return", () => {
    const { s, sent } = session();
    const root = document.createElement("div");
    const close = openChat(root, s, "m:away", "a-away", () => {});
    root.querySelector("textarea")!.value = "do it";
    root.querySelector("form")!.dispatchEvent(new Event("submit", { cancelable: true }));
    close();

    window.dispatchEvent(new CustomEvent("remote-message", { detail: { peer: "m:away", msg: { type: "error", code: "no-agent", message: "That agent isn't open on this computer." } } }));

    const back = document.createElement("div");
    closers.push(openChat(back, s, "m:away", "a-away", () => {}));
    expect(back.querySelector(".pending")?.textContent).toBe("do it (That agent isn't open on this computer.)Send again");
    back.querySelector<HTMLButtonElement>(".pending button")!.click();
    expect(sent.filter((m) => m.type === "chat.send")).toHaveLength(2);
  });

  it("does not take a no-agent error of one computer for a message to another", () => {
    const { s } = session();
    const root = document.createElement("div");
    closers.push(openChat(root, s, "m:other", "a-other", () => {}));
    root.querySelector("textarea")!.value = "do it";
    root.querySelector("form")!.dispatchEvent(new Event("submit", { cancelable: true }));
    hear({ type: "error", code: "no-agent", message: "That agent isn't open on this computer." });
    expect(root.querySelector(".pending")?.textContent).toBe("do it (waiting)");
  });

  it("says a no-agent error in the conversation when no message was waiting", () => {
    const { s } = session();
    const root = document.createElement("div");
    closers.push(openChat(root, s, "m:1", "a-noagent2", () => {}));
    hear({ type: "error", code: "no-agent", message: "That agent isn't open on this computer." });
    expect(root.querySelector(".log")?.textContent).toContain("That agent isn't open on this computer.");
  });

  it("leaves items that did not change alone on an update", () => {
    const { s } = session();
    const root = document.createElement("div");
    closers.push(openChat(root, s, "m:1", "a-keyed", () => {}));
    hear({ type: "chat.items", agent: "a-keyed", older: true, partial: false, live: false, reset: false, items: [t("k1", "one"), t("k2", "two")] });
    const [one, two] = root.querySelectorAll(".item");

    hear({ type: "chat.items", agent: "a-keyed", older: false, partial: false, live: true, reset: false, items: [t("k2", "two"), t("k3", "three")] });
    const after = [...root.querySelectorAll(".item")];
    expect(after.map((e) => e.textContent?.trim())).toEqual(["one", "two", "three"]);
    expect(after[0]).toBe(one);
    expect(after[1]).toBe(two);

    // An item that moved on is drawn again, in its place; an older page goes in front.
    hear({ type: "chat.items", agent: "a-keyed", older: false, partial: false, live: true, reset: false, items: [t("k2", "two, longer")] });
    hear({ type: "chat.items", agent: "a-keyed", older: false, partial: false, live: false, reset: false, items: [t("k0", "zero")] });
    const final = [...root.querySelectorAll(".item")];
    expect(final.map((e) => e.textContent?.trim())).toEqual(["zero", "one", "two, longer", "three"]);
    expect(final[1]).toBe(one);
  });

  it("says when a conversation is empty, and keeps the words out of the way once it isn't", () => {
    const { s } = session();
    const root = document.createElement("div");
    closers.push(openChat(root, s, "m:1", "a-empty", () => {}));
    expect(root.querySelector(".log")?.textContent).toContain("Loading the conversation");
    hear({ type: "chat.items", agent: "a-empty", older: false, partial: false, live: false, reset: false, items: [] });
    expect(root.querySelector(".log")?.textContent).toContain("No messages yet");
    hear({ type: "chat.items", agent: "a-empty", older: false, partial: false, live: true, reset: false, items: [t("e1", "hi")] });
    expect(root.querySelector(".log")?.textContent?.trim()).toBe("hi");
  });

  it("keeps a message that did not get through as not sent, across closing the page", () => {
    const first = session();
    const root = document.createElement("div");
    const close = openChat(root, first.s, "m:1", "a-keep", () => {});
    root.querySelector("textarea")!.value = "do it";
    root.querySelector("form")!.dispatchEvent(new Event("submit", { cancelable: true }));
    expect(first.sent.at(-1)).toMatchObject({ type: "chat.send", agent: "a-keep", text: "do it" });
    expect(root.querySelector(".pending")?.textContent).toBe("do it (waiting)");
    close();

    // The line came back: a new session, a new page.
    const second = session();
    const again = document.createElement("div");
    closers.push(openChat(again, second.s, "m:1", "a-keep", () => {}));
    expect(again.querySelector(".pending")?.textContent).toBe("do it (not sent)Send again");
    expect(second.sent.some((m) => m.type === "chat.send")).toBe(false);
    again.querySelector<HTMLButtonElement>(".pending button")!.click();
    expect(second.sent.filter((m) => m.type === "chat.send")).toHaveLength(1);
    // Same id as the first try, so the computer can tell it is the same message.
    const firstSend = first.sent.find((m) => m.type === "chat.send") as Extract<ToMachine, { type: "chat.send" }>;
    expect(second.sent.find((m) => m.type === "chat.send")).toMatchObject({ id: firstSend.id });
  });
});
