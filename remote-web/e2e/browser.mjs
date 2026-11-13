// The page itself, in headless Chrome: DEV_LOGIN sign-in through the real button,
// pairing by typing the code into the real form (a peers message on the way keeping
// what was typed), the machine's name on the page, a new device's code and QR, a
// reload that reconnects with no code, the live terminal of an agent (sample
// output, typing and its echo, leaving the tab, the one button that resizes), the
// conversation of the agent (history, Show earlier, a message going waiting then typed
// then gone, a message surviving a tab switch, a message the dead line never carried
// staying "not sent" until Send again, a refused message saying why, a permission
// question as buttons, markup in an agent's text running nothing), a
// refused login, a removed device with two tabs of one browser, a framed page, and
// notifications: the service worker registers under the CSP, a click subscribes the
// browser and gives the relay its endpoint, the computer leaves a sealed note for a
// device that is away, and the worker opens it with the vault (a note sealed for
// another device, or from a computer never paired, shows only the plain line).
// Run once against `vite dev` and once against `vite preview` of a
// production build. No console error, page error or CSP violation is allowed,
// except the 401 the refused-login step makes on purpose.
//
//   1. relay/    npm run dev                       (needs DEV_LOGIN=1 in .dev.vars)
//   2. src-tauri cargo build --example remote_machine
//   3. remote-web node e2e/browser.mjs
//
// BYTEHIVE_RELAY, MACHINE_EXE and CHROME override the relay, the machine binary
// and Chrome. The page is served on 17640 because the relay sends logins there; if
// that port is taken, APP_PORT moves it and the relay must be started with the same
// address (wrangler dev --var APP_ORIGIN:http://127.0.0.1:<port>).

import { spawn, spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import http from "node:http";
import path from "node:path";
import { pathToFileURL } from "node:url";
import puppeteer from "puppeteer-core";
import { APP_PORT, RELAY, RELAY_WS, check, root, sleep, startMachine, waitFor, web } from "./lib.mjs";

const APP = `http://127.0.0.1:${APP_PORT}`;
const CHROME = process.env.CHROME ?? "C:/Program Files/Google/Chrome/Application/chrome.exe";
const VITE = path.join(web, "node_modules", "vite", "bin", "vite.js");
const env = { ...process.env, VITE_RELAY: RELAY, VITE_RELAY_WS: RELAY_WS };

/** The dev account of the relay's DEV_LOGIN: what a click on the real button signs in as. */
const ACCOUNT = "dev";

function serve(args) {
  const child = spawn(process.execPath, [VITE, ...args, "--port", String(APP_PORT), "--strictPort", "--host", "127.0.0.1"], { cwd: web, env, stdio: "ignore" });
  return child;
}
async function up() {
  for (let i = 0; i < 150; i++) {
    if (await fetch(APP).then((r) => r.ok, () => false)) return;
    await sleep(100);
  }
  throw new Error("the page server never came up");
}

/** Everything that would be a defect, per page. */
function watch(page) {
  const bad = [];
  page.on("console", (m) => { if (m.type() === "error" || m.type() === "warning") bad.push(`console ${m.type()}: ${m.text()}`); });
  page.on("response", (r) => { if (r.status() >= 400) bad.push(`response ${r.status()}: ${r.url()}`); });
  page.on("pageerror", (e) => bad.push(`page error: ${e.message}`));
  page.on("requestfailed", (r) => bad.push(`request failed: ${r.url()} ${r.failure()?.errorText}`));
  page.evaluateOnNewDocument(() => {
    document.addEventListener("securitypolicyviolation", (e) => console.error(`CSP violation: ${e.violatedDirective} ${e.blockedURI}`));
  });
  return bad;
}
const clean = (bad, what, allow = []) => {
  const left = bad.filter((b) => !allow.some((a) => b.includes(a)));
  bad.length = 0;
  check(left.length === 0, `${what}: nothing in the console${left.length ? ` (${left.join(" | ")})` : ""}`);
};

const clickButton = (page, label) => page.evaluate((l) => {
  const b = [...document.querySelectorAll("button")].find((x) => x.textContent === l);
  if (!b) throw new Error(`no button "${l}"`);
  b.click();
}, label);
/** The rows of the terminal on the page, as text. */
const screen = (page) => page.$$eval(".xterm-rows > div", (rows) => rows.map((r) => r.textContent.replace(/\p{Zs}/gu, " ")).join("\n")).catch(() => "");
async function screenHas(page, word, what) {
  const end = Date.now() + 10_000;
  while (Date.now() < end) {
    if ((await screen(page)).includes(word)) return;
    await sleep(50);
  }
  throw new Error(`timed out waiting for ${what}; the terminal shows: ${JSON.stringify(await screen(page))}`);
}

const text = (page, sel) => page.$eval(sel, (e) => e.textContent).catch(() => null);
const until =(page, fn, what, arg, ms = 10_000) => page.waitForFunction(fn, { timeout: ms, polling: 100 }, arg).catch(async () => {
  const shown = await page.evaluate(() => document.body.innerText).catch(() => "?");
  throw new Error(`timed out waiting for ${what}; the page shows: ${JSON.stringify(shown)}`);
});
/** For what the machine says unasked (`machine.say`): it leaves the machine within a poll, so a few seconds is generous. */
const heard = (page, fn, what) => until(page, fn, what, undefined, 5_000);
/** For a socket the page closed itself: the relay's room has no close handler, so the close handshake takes its time. */
const late = (page, fn, what) => until(page, fn, what, undefined, 30_000);

/** The agent the machine reports, as a row of its project with its state in words. */
const fleetShown = () => [...document.querySelectorAll(".project")].some((p) => p.querySelector("h3")?.textContent === "demo"
  && [...p.querySelectorAll(".agent")].some((a) => a.querySelector(".agent-name")?.textContent === "claude" && a.querySelector(".agent-status")?.textContent === "Working"));

/** What this browser keeps under a key of the page's own storage. */
function stored(page, key) {
  return page.evaluate((k) => new Promise((res, rej) => {
    const r = indexedDB.open("bytehive-remote", 1);
    r.onerror = () => rej(r.error);
    r.onsuccess = () => {
      const g = r.result.transaction("kv").objectStore("kv").get(k);
      g.onsuccess = () => res(g.result ?? null);
    };
  }), key);
}
const storedDeviceId = async (page) => (await stored(page, "device"))?.id ?? null;

async function storeToken(page, value) {
  await page.evaluate((v) => new Promise((res, rej) => {
    const r = indexedDB.open("bytehive-remote", 1);
    r.onerror = () => rej(r.error);
    r.onsuccess = () => {
      const t = r.result.transaction("kv", "readwrite");
      t.objectStore("kv").put(v, "token");
      t.oncomplete = () => res();
    };
  }), value);
}

/** Connects a machine of the dev account of its own, as the relay sees one. */
async function dial(peer, name) {
  const login = await fetch(`${RELAY}/auth/start?peer=${peer}&port=17999&state=x&account=${ACCOUNT}`, { redirect: "manual" });
  const token = new URL(login.headers.get("location")).searchParams.get("token");
  const { ticket } = await (await fetch(`${RELAY}/ticket`, { method: "POST", headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" }, body: JSON.stringify({ name }) })).json();
  const ws = new WebSocket(`${RELAY_WS}/ws?ticket=${encodeURIComponent(ticket)}`);
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = () => rej(new Error(`${name} could not connect`)); });
  return ws;
}

/** Revokes a device the way a machine does: its own socket, a `revoke` text message. */
async function revokeAsMachine(deviceId) {
  const ws = await dial("m:revoker", "Revoker");
  ws.send(JSON.stringify({ t: "revoke", peer: `d:${deviceId}` }));
  await sleep(300);
  ws.close();
}

async function run(browser, machine, label) {
  console.log(`\n== ${label}`);
  const context = await browser.createBrowserContext();
  const page = await context.newPage();
  const bad = watch(page);
  // Lets a run take the relay's socket down, or leave it up but deaf, like a line that has died.
  await page.evaluateOnNewDocument(() => {
    window.__sockets = [];
    const Native = window.WebSocket;
    window.WebSocket = class extends Native {
      constructor(...a) { super(...a); window.__sockets.push(this); }
      send(d) { if (!window.__lineDown) super.send(d); }
    };
  });

  // Sign in through the real button.
  await page.goto(APP);
  await until(page, () => document.querySelector("button")?.textContent === "Sign in with GitHub", "the sign-in button");
  await Promise.all([page.waitForNavigation({ waitUntil: "load" }), page.click("button")]);
  await until(page, () => [...document.querySelectorAll("a")].some((a) => a.textContent === "Pair a computer"), "the home page after sign-in");
  check(!page.url().includes("token"), "the login leaves the address at once");
  clean(bad, "sign-in");

  // Pair by typing the code into the real form.
  await page.click('a[href="#/pair"]');
  await until(page, (n) => [...document.querySelectorAll("#m option")].some((o) => o.textContent === n), "the machine in the list", machine.name);
  const code = await machine.newCode();
  await page.type("#c", code.slice(0, 3));
  const passer = await dial("m:passer", "Passer-by");
  await until(page, () => [...document.querySelectorAll("#m option")].some((o) => o.textContent === "Passer-by"), "a computer coming online during the typing");
  check((await page.$eval("#c", (i) => i.value)) === code.slice(0, 3), "a peers message leaves a half-typed code alone");
  check((await page.$eval("#m", (s) => s.value)) === machine.id, "and the computer that was chosen");
  passer.close();
  await page.type("#c", code.slice(3));
  await page.click("#go");
  await until(page, (n) => document.querySelector("h1")?.textContent === n, "the machine's name on the page", machine.name);
  check(page.url().endsWith(`#/m/${machine.id}`), "after pairing the page moves to the machine");
  await until(page, fleetShown, "the fleet");
  check(true, `the page shows "${await text(page, "h1")}" and its agent claude, working, under demo`);
  // The example machine makes codes on its stdin only, so it answers the button by saying one.
  await clickButton(page, "Add a device");
  machine.say({ type: "pair.code", code: "E2E-CODE", expires: Date.now() + 600_000 });
  await heard(page, () => document.querySelector(".invite-code")?.textContent === "E2E-CODE", "the code of a new device");
  await until(page, () => {
    const c = document.querySelector(".invite canvas");
    return c?.width > 0 && c.getContext("2d").getImageData(0, 0, c.width, c.height).data.some((v, n) => n % 4 === 0 && v < 100);
  }, "the QR drawn next to the code");
  check(true, "Add a device shows the code, and the QR of its link");
  clean(bad, "pairing");

  // A reload: no code, the pinned key.
  await page.reload();
  await until(page, fleetShown, "the fleet after a reload");
  check(true, "a reload reconnects with no code and shows the fleet again");
  clean(bad, "reload");

  // The live terminal of the agent. The machine keeps its record across runs: count from here.
  const base = machine.terms().length;
  const keysBase = machine.keyBytes();
  const terms = () => machine.terms().slice(base);
  await page.click(`a[href="#/m/${machine.id}/a/a1"]`);
  await until(page, () => document.querySelector("h1")?.textContent === "demo / claude", "the agent page");
  check(JSON.stringify(terms()) === "[]", "the agent page opens on Chat and asks the machine for no terminal");
  await clickButton(page, "Terminal");
  await screenHas(page, "earlier output", "the sample output in the terminal");
  check(JSON.stringify(terms()) === '["open a1"]', "the Terminal tab opens the agent's terminal once");
  check((await page.$$eval(".xterm-rows > div", (r) => r.length)) === 24, "it opens at the agent's own size (24 rows), not the screen's");
  // The replay holds device attribute, cursor position and colour queries: the page must not answer them.
  await sleep(500);
  check(machine.keyBytes() === keysBase, "the replayed queries (DA, CPR, OSC 11) get no answer sent as keys");
  await page.click(".xterm");
  await page.keyboard.type("hi");
  await screenHas(page, "$ hi", "the typed keys echoed back");
  check(true, "keys typed on the page reach the machine and come back on the screen");
  // A phone keyboard has no Up or Esc: the row above the terminal sends them.
  await page.setViewport({ width: 390, height: 844 });
  const keysBefore = machine.keyBytes();
  await until(page, () => [...document.querySelectorAll(".term-keys button")].some((b) => b.textContent === "Up" && b.offsetWidth >= 44 && b.offsetHeight >= 44), "the row of keys on a narrow screen");
  await page.click(".term-keys button:nth-child(3)");
  await waitFor(() => machine.keyBytes() === keysBefore + 3, "the machine to hear the Up key");
  check(true, "the Up key of the row reaches the machine as its three bytes");
  await page.setViewport({ width: 800, height: 600 });
  await until(page, () => !document.querySelector(".term-keys")?.offsetWidth, "the row gone on a wide screen with a mouse");
  check(true, "and the row is not there on a wide screen with a mouse");

  // Anything that re-draws the page, a fleet update included, must keep the terminal.
  await page.evaluate(() => window.dispatchEvent(new HashChangeEvent("hashchange")));
  await sleep(300);
  check(JSON.stringify(terms()) === '["open a1"]', "a re-draw of the page does not close and reopen the terminal");
  check((await screen(page)).includes("$ hi"), "and keeps what is on the screen");
  await clickButton(page, "Fit to this screen");
  await waitFor(() => terms().length === 2, "the resize");
  check(/^resize a1 \d+x\d+$/.test(terms()[1]), `only the button resizes the machine's terminal (${terms()[1]})`);
  await clickButton(page, "Chat");
  await waitFor(() => terms().at(-1) === "close a1", "the close");
  check(!(await page.$(".xterm")), "leaving the Terminal tab closes it on the machine and on the page");
  await clickButton(page, "Terminal");
  await screenHas(page, "earlier output", "the output again after reopening");
  check(terms().at(-1) === "open a1", "opening it again replays what the machine has");
  await page.click(`a[href="#/m/${machine.id}"]`);
  await waitFor(() => terms().at(-1) === "close a1", "the close on leaving the page");
  check(terms().map((t) => t.split(" ")[0]).join(",") === "open,resize,close,open,close" && terms().filter((t) => t.startsWith("resize")).length === 1,
    "leaving the agent page closes the terminal, and the button was the only resize");
  clean(bad, "terminal");

  // The conversation of the agent, from the machine's fixed history.
  const chatBase = machine.chats().length;
  const chats = () => machine.chats().slice(chatBase);
  const sends = () => chats().filter((c) => c.op === "send");
  const promptBase = machine.prompts().length;
  const answers = () => machine.prompts().slice(promptBase);
  const type = async (message) => {
    await page.type("textarea", message);
    await clickButton(page, "Send");
  };
  await page.click(`a[href="#/m/${machine.id}/a/a1"]`);
  await until(page, () => document.querySelectorAll(".log .item").length === 40, "the newest 40 items of the history");
  check(chats().length === 1 && chats()[0].op === "page" && chats()[0].before === null, "opening an agent asks for the newest page of its conversation, once");
  check((await page.$$eval("button", (b) => b.some((x) => x.textContent === "Show earlier"))), "an older page is offered");
  check((await text(page, ".log .item:last-child strong")) === "safe words", "an agent's markdown is drawn (bold words)");
  check((await page.evaluate(() => window.__ran)) === undefined, "markup in an agent's text ran nothing (<img onerror>, <script>, javascript: link)");
  check((await page.$$eval(".log img, .log script, .log a[href^=javascript]", (l) => l.length)) === 0, "and left no picture, script or script link on the page");
  check((await text(page, ".log .item:last-child")).includes("<script>window.__ran=2</script>"), "the markup an agent typed is shown as the characters it was written with");
  clean(bad, "chat history");
  await clickButton(page, "Show earlier");
  await until(page, () => document.querySelectorAll(".log .item").length === 45, "the earlier items");
  check(chats().at(-1).op === "page" && chats().at(-1).before === "h06", "Show earlier asks for the page before the oldest item it has");
  check((await text(page, ".log .item:first-of-type")).trim() === "question 1", "the older items go in front, in order");
  check(!(await page.$$eval("button", (b) => b.some((x) => x.textContent === "Show earlier"))), "and the button is gone once there is nothing older");

  // A message goes waiting, then typed, then is gone once the transcript has it.
  await type("ping the build");
  await until(page, () => document.querySelector(".pending")?.textContent === "ping the build (waiting)", "the message waiting");
  const first = await waitFor(() => sends()[0], "the machine to hear the message");
  check(first.agent === "a1" && first.n === 14, "the machine heard one message of the right length");
  const pushed = Date.now();
  machine.say({ type: "chat.ack", id: first.id, state: "typed" });
  await heard(page, () => document.querySelector(".pending")?.textContent === "ping the build (typed)", "the message typed");
  check(Date.now() - pushed < 2_000, `a message the machine pushes on an idle link reaches the page at once (${Date.now() - pushed} ms)`);
  machine.say({ type: "chat.items", agent: "a1", items: [{ kind: "user", id: "h46", text: "ping the build", images: 0, at: 46 }], older: false, partial: false, live: true, reset: false });
  machine.say({ type: "chat.ack", id: first.id, state: "seen" });
  await heard(page, () => !document.querySelector(".pending") && document.querySelectorAll(".log .item").length === 46, "the message gone from the queue and in the conversation");
  check((await text(page, ".log .item:last-child")) === "ping the build", "waiting, typed, then gone, and the conversation shows it");
  clean(bad, "chat send");

  // The Terminal tab opens with a message on its way, on a line that is fine: it is not "not sent".
  await type("mind the tabs");
  const onItsWay = await waitFor(() => sends()[1], "the machine to hear the message");
  await clickButton(page, "Terminal");
  await until(page, () => document.querySelector(".xterm"), "the terminal");
  await clickButton(page, "Chat");
  await until(page, () => document.querySelector(".pending")?.textContent === "mind the tabs (waiting)", "the message still waiting after a tab switch");
  await clickButton(page, "Terminal");
  await until(page, () => document.querySelector(".xterm"), "the terminal again");
  machine.say({ type: "chat.ack", id: onItsWay.id, state: "typed" });
  await sleep(1000);
  await clickButton(page, "Chat");
  await until(page, () => document.querySelector(".log .item"), "the conversation again");
  check(!(await page.$(".pending")), "and the answer it got while the Terminal tab was open counts: typed, so the conversation shows it");
  check(sends().length === 2, "a tab switch sends nothing again");

  // Another parse of the conversation (a new one, a whole load): ids mean other items now, so the page starts over.
  machine.say({ type: "chat.items", agent: "a1", items: [{ kind: "text", id: "h06", text: "fresh start", at: 1 }], older: false, partial: false, live: true, reset: true });
  await heard(page, () => document.querySelectorAll(".log .item").length === 1 && document.querySelector(".log .item").textContent.trim() === "fresh start", "the conversation replaced");
  check(true, "a reset replaces the conversation on the page instead of merging by id (h06 was another message before)");

  // The line goes down with a message on its way: it stays "not sent" and nothing types it until Send again.
  await page.evaluate(() => { window.__lineDown = true; });
  await type("one more");
  await until(page, () => document.querySelector(".pending")?.textContent === "one more (waiting)", "the second message waiting");
  await sleep(500);
  check(sends().length === 2, "a message sent into a dead line never reaches the machine");
  await page.evaluate(() => {
    const live = window.__sockets.filter((w) => w.url.includes("/ws?ticket="));
    window.__lineDown = false;
    live.forEach((w) => w.close());
  });
  await late(page, () => document.querySelector(".pending")?.textContent === "one more (not sent)Send again", "the message not sent, after the line came back");
  check(sends().length === 2, "back on the line, the message is not typed by itself");
  await waitFor(() => chats().at(-1)?.op === "page", "the page to ask for the conversation again");
  check(true, "the page came back and asked for the conversation again");
  await clickButton(page, "Send again");
  const second = await waitFor(() => sends()[2], "the machine to hear it after Send again");
  await sleep(300);
  check(sends().length === 3 && second.n === 8 && second.id !== first.id, "Send again sends it once");
  machine.say({ type: "chat.ack", id: second.id, state: "typed" });
  await heard(page, () => document.querySelector(".pending")?.textContent === "one more (typed)", "the second message typed");
  machine.say({ type: "chat.ack", id: second.id, state: "seen" });
  await heard(page, () => !document.querySelector(".pending"), "the second message gone");
  clean(bad, "chat after the line dropped", ["WebSocket"]);

  // The computer has no such agent any more: the message says so, it does not wait for ever.
  const gone = "That agent isn't open on this computer.";
  await page.type("textarea", "anyone there");
  await page.keyboard.press("Enter");
  const refused = await waitFor(() => sends()[3], "the machine to hear the message");
  machine.say({ type: "error", code: "no-agent", message: gone });
  await until(page, (g) => document.querySelector(".pending")?.textContent === `anyone there (${g})Send again`, "the refusal on the message", gone, 5_000);
  check(true, "a no-agent error shows on the message that was waiting");
  machine.say({ type: "chat.ack", id: refused.id, state: "seen" });
  await heard(page, () => !document.querySelector(".pending"), "the refused message gone");

  // A permission question: real buttons, and a press sends the key.
  const asked = { type: "prompt.ask", agent: "a1", id: "q1", screen: "Do you want to proceed?\n1. Yes\n2. No" };
  machine.say({ ...asked, options: [{ label: "Yes", keys: "1" }, { label: "No", keys: "3" }] });
  await heard(page, () => [...document.querySelectorAll(".ask button")].map((b) => b.textContent).join() === "Yes,No", "the question as buttons");
  await page.evaluate(() => [...document.querySelectorAll(".ask button")].find((b) => b.textContent === "No").click());
  await waitFor(() => answers().length === 1, "the machine to hear the answer");
  check(answers()[0].keys === "3" && answers()[0].id === "q1", "pressing a button sends the key of that answer, and the id of the question it answers");
  check((await page.$$eval(".ask button", (b) => b.length)) === 0, "and the buttons go away");
  machine.say({ ...asked, id: "q2", options: [{ label: "Yes", keys: "1" }] });
  await heard(page, () => document.querySelector(".ask button")?.textContent === "Yes", "the question again");
  machine.say({ type: "prompt.clear", agent: "a1" });
  await heard(page, () => document.querySelectorAll(".ask button").length === 0, "the buttons gone when the agent stopped asking");
  check(answers().length === 1, "and nothing was pressed for it");
  machine.say({ ...asked, id: "q3", screen: "Something this page does not know", options: [] });
  await heard(page, () => document.querySelector(".ask button")?.textContent === "Open the terminal to answer", "the terminal offered for a question it can't read");
  check(answers().length === 1, "a question it doesn't know gets no button that guesses");
  await clickButton(page, "Open the terminal to answer");
  await until(page, () => document.querySelector(".xterm"), "the terminal after the offer");
  check(chats().at(-1).op === "close", "leaving the Chat tab closes the conversation on the machine");
  await page.click(`a[href="#/m/${machine.id}"]`);
  await waitFor(() => terms().at(-1) === "close a1", "the terminal closed on leaving the page");
  clean(bad, "chat prompts");

  // A refused login: the stored token is no good.
  await storeToken(page, "garbage");
  await page.reload();
  await until(page, () => document.querySelector("p")?.textContent === "Please sign in again." && document.querySelector("button"), "the way back in");
  check((await text(page, "button")) === "Sign in with GitHub", "a refused login shows the sign-in button and says so");
  await sleep(1500);
  check((await text(page, "p")) === "Please sign in again.", "and nothing draws over it");
  clean(bad, "refused login", ["401"]);
  await Promise.all([page.waitForNavigation({ waitUntil: "load" }), page.click("button")]);
  await until(page, () => [...document.querySelectorAll("a")].some((a) => a.textContent === "Pair a computer"), "the home page after signing in again");
  check(true, "signing in again works, and the paired machine is still there");
  await until(page, () => [...document.querySelectorAll(".machine h2")].some((h) => h.textContent.startsWith("E2E Desk")), "the machine in the list");
  clean(bad, "second sign-in");

  // The machine removes this device while the page is open, and a second tab of
  // the same browser (same storage) is open too.
  const tabB = await context.newPage();
  const badB = watch(tabB);
  await tabB.goto(APP);
  await until(tabB, () => [...document.querySelectorAll("a")].some((a) => a.textContent === "Pair a computer"), "the second tab's home page");
  // One device id, one socket: the relay closed the first tab's with 4000. It must
  // stop there and ask, not take the socket back (the tabs would swap it every second).
  const parked = () => document.querySelector("p")?.textContent === "Open in another tab." && document.querySelector("button")?.textContent === "Use this tab";
  await until(page, parked, "the first tab saying another tab has the connection");
  await sleep(2500);
  check(await page.evaluate(parked), "the first tab stays parked: the two tabs do not take the connection from each other");
  check(await tabB.evaluate(() => [...document.querySelectorAll("a")].some((a) => a.textContent === "Pair a computer")), "and the second tab keeps its connection");
  await clickButton(page, "Use this tab");
  await until(page, () => [...document.querySelectorAll("a")].some((a) => a.textContent === "Pair a computer"), "the first tab after it took the connection back");
  await until(tabB, parked, "the second tab, now the parked one");
  check(true, "the button takes the connection back, and the other tab is the one told");
  const before = await storedDeviceId(page);
  const tokenBefore = await stored(page, "token");
  await revokeAsMachine(before);
  const removed = () => document.querySelector("p")?.textContent === "This device was removed. Sign in and pair it again." && document.querySelector("button");
  await until(page, removed, "the removed message");
  // The parked tab hears of the removal when it asks to connect.
  await clickButton(tabB, "Use this tab");
  await until(tabB, removed, "the removed message in the second tab");
  check((await text(page, "button")) === "Sign in with GitHub", "a removed device is told so and offered the sign-in button");
  check((await storedDeviceId(page)) === before && (await stored(page, "token")) === tokenBefore && (await stored(page, "machines"))?.length === 1,
    "the removal itself erased nothing: identity, login and paired machine are all still stored");

  // The first tab starts over: a new identity, a new login, no machine kept.
  await page.bringToFront();
  await Promise.all([page.waitForNavigation({ waitUntil: "load" }), page.click("button")]);
  await until(page, () => [...document.querySelectorAll("a")].some((a) => a.textContent === "Pair a computer"), "the home page with a new identity");
  const fresh = await storedDeviceId(page);
  check(fresh && fresh !== before, "signing in after a removal is a different device");
  check((await stored(page, "machines")) === null || (await stored(page, "machines")).length === 0, "and it keeps no machine of the old identity");
  await sleep(500);
  check(await page.$$eval(".machine", (l) => l.length) === 0, "so the home page does not list a machine it cannot reach");

  // Pair again with a new code: proof the removed device can come back.
  await page.click('a[href="#/pair"]');
  await until(page, (n) => [...document.querySelectorAll("#m option")].some((o) => o.textContent === n), "the machine in the list again", machine.name);
  await page.type("#c", await machine.newCode());
  await page.click("#go");
  await until(page, (n) => document.querySelector("h1")?.textContent === n, "the machine's name after pairing again", machine.name);
  await until(page, fleetShown, "the fleet after pairing again");
  check(true, "a removed device pairs again with a new code and shows the fleet");

  // The second tab, still holding the old identity, must not erase the new one.
  await tabB.bringToFront();
  await Promise.all([tabB.waitForNavigation({ waitUntil: "load" }), tabB.click("button")]);
  await until(tabB, () => [...document.querySelectorAll("a")].some((a) => a.textContent === "Pair a computer"), "the second tab after its reload");
  check((await storedDeviceId(tabB)) === fresh, "the stale tab reloaded onto the new identity instead of erasing it");
  check((await stored(tabB, "machines"))?.length === 1, "and the machine paired again is still there");
  await tabB.close();
  clean(bad, "removed device", ["403", "4001", "WebSocket"]);
  clean(badB, "stale tab", ["403", "4001", "WebSocket"]);

  // Framed by another origin: nothing but a message.
  const host = http.createServer((_, res) => { res.setHeader("Content-Type", "text/html"); res.end(`<iframe src="${APP}"></iframe>`); });
  await new Promise((r) => host.listen(17650, "127.0.0.1", r));
  try {
    const outer = await context.newPage();
    await outer.goto("http://127.0.0.1:17650/");
    const inner = await waitFor(() => outer.frames().find((f) => f.url().startsWith(APP)), "the framed page");
    await inner.waitForFunction(() => document.body.innerText.length > 0, { timeout: 10_000 });
    const framed = await inner.evaluate(() => document.body.innerText);
    check(framed.includes("can't run inside another page") && !framed.includes("Sign in"), "inside a frame of another origin the page shows only a message");
  } finally {
    host.close();
  }

  await context.close();
}

/** Waits for something only the node side can look at. */
async function eventually(read, what, ms = 10_000) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    const got = await read().catch(() => undefined);
    if (got) return got;
    await sleep(100);
  }
  throw new Error(`timed out waiting for ${what}`);
}

/** Adds a paired computer to what this browser keeps, with the vault it was given as bytes. */
function pairWith(page, id, vault) {
  return page.evaluate((id, bytes) => new Promise((res, rej) => {
    const r = indexedDB.open("bytehive-remote", 1);
    r.onerror = () => rej(r.error);
    r.onsuccess = () => {
      const t = r.result.transaction("kv", "readwrite");
      const kv = t.objectStore("kv");
      kv.get("machines").onsuccess = (e) => kv.put([...(e.target.result ?? []), { id, name: id, key: new Uint8Array(32), vault: Uint8Array.from(bytes) }], "machines");
      t.oncomplete = () => res();
    };
  }), id, [...vault]);
}

/** Every worker, subscription and notification of the page's origin, gone. */
const forgetWorker = (page) => page.evaluate(() => navigator.serviceWorker.getRegistrations().then((rs) => Promise.all(rs.map(async (r) => {
  for (const n of await r.getNotifications()) n.close();
  await (await r.pushManager.getSubscription())?.unsubscribe();
  await r.unregister();
}))));

/**
 * Notifications, in the browser's own profile: Chrome gives push to no private window.
 * A device that is not connected is woken, the service worker fetches what the machine
 * left sealed and opens it with the vault; a note that is not for this device, or from a
 * computer it never paired with, shows only the plain line. The push comes through the
 * real push service when it answers within a few seconds, and is delivered through the
 * browser's debugging protocol when it does not; the output says which.
 */
async function notifications(browser, machine, label) {
  console.log(`\n== ${label}`);
  const noise = await import(pathToFileURL(path.join(root, "remote", "noise-wasm", "pkg", "bytehive_noise_wasm.js")).href);
  await noise.default({ module_or_path: readFileSync(path.join(root, "remote", "noise-wasm", "pkg", "bytehive_noise_wasm_bg.wasm")) });
  const PLAIN = { title: "ByteHive", body: "There's an update on your agents." };

  // A worker an earlier run left behind (dev, then a build, at the same address) is not the one to watch.
  const left = new Set(browser.targets().filter((t) => t.type() === "service_worker"));
  const context = browser.defaultBrowserContext();
  const grants = await browser.target().createCDPSession();
  for (const permission of [{ name: "notifications" }, { name: "push", userVisibleOnly: true }]) {
    await grants.send("Browser.setPermission", { permission, setting: "granted", origin: APP });
  }
  const page = await context.newPage();
  const bad = watch(page);
  let driver = null;
  const closers = [];
  try {
    // A clean start: the default profile keeps what an earlier run left.
    await page.goto(`${APP}/manifest.webmanifest`);
    await forgetWorker(page);
    await (await page.createCDPSession()).send("Storage.clearDataForOrigin", { origin: APP, storageTypes: "all" });
    // A page that is not HTML asks for a favicon the site does not have: not the page under test.
    bad.length = 0;

    await page.goto(APP);
    await until(page, () => document.querySelector("button")?.textContent === "Sign in with GitHub", "the sign-in button");
    await Promise.all([page.waitForNavigation({ waitUntil: "load" }), page.click("button")]);
    await until(page, () => [...document.querySelectorAll("a")].some((a) => a.textContent === "Pair a computer"), "the home page after sign-in");
    await page.click('a[href="#/pair"]');
    await until(page, (n) => [...document.querySelectorAll("#m option")].some((o) => o.textContent === n), "the machine in the list", machine.name);
    await page.type("#c", await machine.newCode());
    await page.click("#go");
    await until(page, fleetShown, "the fleet");
    await page.evaluate(() => { location.hash = "#/"; });
    await until(page, () => document.querySelector("#notify")?.textContent === "Turn on notifications", "the notifications button on the home page");
    check(true, "a paired device is offered Turn on notifications");

    // Turn on, as the person does: a click. The worker registers under the page's CSP.
    await page.click("#notify");
    await until(page, () => document.querySelector(".notify")?.textContent === "Notifications on", "notifications on", undefined, 20_000);
    const reg = await page.evaluate(async () => {
      const r = await navigator.serviceWorker.getRegistration("/");
      return { script: new URL(r.active.scriptURL).pathname, scope: new URL(r.scope).pathname, endpoint: (await r.pushManager.getSubscription()).endpoint };
    });
    check(reg.script === "/sw.js" && reg.scope === "/", "the service worker registered at /sw.js for the whole site, under the CSP");
    check(/^https:\/\//.test(reg.endpoint), `the browser subscribed (${new URL(reg.endpoint).hostname}) and the relay took the endpoint`);
    clean(bad, "turning notifications on");

    // What the page keeps for the worker: a login, its own id and the machine's vault.
    const token = await stored(page, "token");
    const device = await storedDeviceId(page);
    // Bytes come back from the page as an object keyed by position.
    const vault = Uint8Array.from(Object.values(await eventually(async () => (await stored(page, "machines"))?.[0]?.vault, "the vault from the computer")));
    check(vault.length === 32, "the computer gave the page its vault key");
    const pending = async () => (await fetch(`${RELAY}/push/pending`, { method: "POST", headers: { Authorization: `Bearer ${token}` } })).json();

    // A device that is connected hears the computer itself: nothing is left for it.
    machine.note("While you look", "Needs you");
    await sleep(500);
    check((await pending()) === null, "a note for a device that is connected is not kept for its browser");

    // The device goes away; the relay's list of peers is how the computer finds out.
    const watching = await dial("m:watcher", "Watcher");
    closers.push(() => watching.close());
    let peers = [];
    watching.onmessage = (e) => { if (typeof e.data === "string") { const m = JSON.parse(e.data); if (m.t === "peers") peers = m.peers; } };
    await page.close();
    await eventually(async () => peers.some((p) => p.id === `d:${device}` && !p.online), "the relay to list the device as away");
    await sleep(300);

    // A page of the same origin that runs none of the app: it reads the worker's notifications and gives the worker its pushes.
    driver = await context.newPage();
    await driver.goto(`${APP}/manifest.webmanifest`);
    // A computer this device has the vault of, but that is not the one that made the note.
    await pairWith(driver, "forger", vault);
    const sw = await driver.createCDPSession();
    const regs = new Map();
    sw.on("ServiceWorker.workerRegistrationUpdated", (e) => e.registrations.forEach((r) => regs.set(r.scopeURL, r.registrationId)));
    await sw.send("ServiceWorker.enable");
    // Headless Chrome shows no notification and keeps none to ask for, so the worker is watched at the
    // one call that shows one. Attached, it also stays alive between the pushes.
    await sw.send("ServiceWorker.startWorker", { scopeURL: `${APP}/` });
    const worker = await (await browser.waitForTarget((t) => t.type() === "service_worker" && t.url() === `${APP}/sw.js` && !left.has(t), { timeout: 10_000 })).worker();
    await worker.evaluate(() => {
      const show = ServiceWorkerRegistration.prototype.showNotification;
      ServiceWorkerRegistration.prototype.showNotification = function (title, options) {
        (self.__shown ??= []).push({ title, body: options?.body });
        return show.call(this, title, options);
      };
    });
    const shown = () => worker.evaluate(() => self.__shown ?? []);
    const clearShown = () => worker.evaluate(() => { self.__shown = []; });
    /** The notifications after one push: the push service's own if it comes, else delivered here. */
    async function afterPush() {
      const got = async () => { const l = await shown(); return l.length ? l : null; };
      let via = "the real push service";
      let list = await eventually(got, "a push", 20_000).catch(() => []);
      if (!list.length) {
        via = "the browser's debugging protocol";
        await eventually(async () => regs.get(`${APP}/`), "the registration id");
        await sw.send("ServiceWorker.deliverPushMessage", { origin: APP, registrationId: regs.get(`${APP}/`), data: "" });
        list = await eventually(got, "the worker's notification", 15_000);
      }
      console.log(`     (the push came through ${via})`);
      return { list, via };
    }

    // The real thing: the computer's own sealed note, through the relay, to the worker.
    await clearShown();
    machine.note("Ana", "Needs you");
    const first = await afterPush();
    check(JSON.stringify(first.list) === JSON.stringify([{ title: "Ana", body: "Needs you" }]), `the notification names the agent and what it wants: ${JSON.stringify(first.list)}`);
    check((await pending()) === null, "and the relay keeps nothing once the worker has taken the note");

    // A note for another device does not open, even from a computer whose vault this device has.
    const as = async (machineId, aadDevice, title, body) => {
      const ws = await dial(`m:${machineId}`, machineId);
      const sealed = noise.seal(vault, new TextEncoder().encode(`note:${aadDevice}`), new TextEncoder().encode(JSON.stringify({ title, body })));
      ws.send(JSON.stringify({ t: "push", peer: `d:${device}`, sealed: Buffer.from(sealed).toString("base64") }));
      await sleep(300);
      ws.close();
    };
    await clearShown();
    await as("forger", "d:someone-else", "Forged", "for another device");
    const swapped = await afterPush();
    check(JSON.stringify(swapped.list) === JSON.stringify([PLAIN]), `a note sealed for another device shows only the plain line: ${JSON.stringify(swapped.list)}`);

    // Proof the forger's vault and format were right: the same note, sealed for this device, opens.
    await clearShown();
    await as("forger", `d:${device}`, "Right", "sealed for this device");
    const right = await afterPush();
    check(JSON.stringify(right.list) === JSON.stringify([{ title: "Right", body: "sealed for this device" }]), `the same note sealed for this device opens: ${JSON.stringify(right.list)}`);

    // A computer this device never paired with has no vault here.
    await clearShown();
    await as("stranger", `d:${device}`, "Stranger", "no vault");
    const stranger = await afterPush();
    check(JSON.stringify(stranger.list) === JSON.stringify([PLAIN]), `a note from a computer that was never paired shows only the plain line: ${JSON.stringify(stranger.list)}`);
    clean(bad, "notifications");
  } finally {
    for (const close of closers) close();
    const last = driver ?? page;
    await forgetWorker(last).catch(() => {});
    await last.createCDPSession().then(async (s) => {
      await s.send("Storage.clearDataForOrigin", { origin: APP, storageTypes: "all" });
      await s.send("ServiceWorker.enable");
      await s.send("ServiceWorker.stopAllWorkers");
    }).catch(() => {});
    await grants.send("Browser.resetPermissions").catch(() => {});
    await driver?.close().catch(() => {});
    await page.close().catch(() => {});
  }
}

let failed = false;
let machine = null;
let server = null;
let browser = null;
try {
  if (!existsSync(CHROME)) throw new Error(`no Chrome at ${CHROME}: set CHROME`);
  machine = await startMachine(ACCOUNT);
  console.log(`machine ${machine.id} (${machine.name}) on ${RELAY}, account ${ACCOUNT}`);
  browser = await puppeteer.launch({ executablePath: CHROME, headless: true });
  console.log(`Chrome ${await browser.version()}`);

  server = serve([]);
  await up();
  await run(browser, machine, "vite dev");
  await notifications(browser, machine, "notifications, vite dev");
  server.kill();
  await sleep(500);

  const build = spawnSync(process.execPath, [VITE, "build", "--logLevel", "error"], { cwd: web, env, encoding: "utf8" });
  if (build.status !== 0) throw new Error(`vite build failed: ${build.stdout}${build.stderr}`);
  server = serve(["preview"]);
  await up();
  await run(browser, machine, "vite preview of a production build");
  await notifications(browser, machine, "notifications, vite preview of a production build");
  console.log("\nPASS");
} catch (e) {
  failed = true;
  console.error(String(e?.stack ?? e));
} finally {
  await browser?.close().catch(() => {});
  server?.kill();
  machine?.stop();
}
process.exit(failed ? 1 : 0);
