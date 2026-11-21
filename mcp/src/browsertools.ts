/* The browser tools, as data: each with its schema, what it does, and a one-line
 * brief for the single `browser` tool (browsergate.ts). server.ts registers
 * them one by one for a client that loads tool schemas only when needed
 * (Claude Code), and as that one tool for the others, which pay for every
 * schema on every request. */

import fs from "node:fs";
import path from "node:path";
import { z } from "zod";
import { type HubClient, type ToolResult, err, recordingPath, saveRecording, uploadPaths } from "./browser.js";

export interface BrowserTool {
  /** The name without the browser_ prefix. */
  name: string;
  description: string;
  /** One line for the single browser tool's description. */
  brief: string;
  inputSchema: Record<string, z.ZodTypeAny>;
  run: (args: Record<string, unknown>) => Promise<ToolResult>;
}

const tabId = z.number().int().optional().describe("Tab id from browser_tabs; defaults to your most recent tab");

export function browserTools(hub: Pick<HubClient, "call">, base = process.cwd(), agent = "Agent"): BrowserTool[] {
  const tools: BrowserTool[] = [];
  const tool = (name: string, description: string, brief: string, inputSchema: Record<string, z.ZodTypeAny>, hubTool: string) =>
    tools.push({ name, description, brief, inputSchema, run: (args) => hub.call(hubTool, args) });

  tool("list", "List the browsers (Chrome/Edge/Brave profiles) connected to ByteHive. The one marked * is the one you use.", "connected browsers, * is yours", {}, "list_browsers");
  tool(
    "select",
    "Choose which connected browser profile you use, by profile name, email or id from browser_list.",
    "use another browser profile, by name, email or id",
    { browser: z.string().describe("Profile name, email, or id") },
    "select_browser",
  );
  tool(
    "tabs",
    "Your tabs in the browser (your tab group). With all=true, also every other open tab, which you can take with browser_tab_adopt.",
    "your tabs; all=true adds every open tab",
    { all: z.boolean().optional() },
    "tabs",
  );
  tool(
    "tab_new",
    "Open another tab in your tab group, optionally at a URL. Only when you need a second page next to the one you have; to go somewhere else, use browser_navigate in your current tab.",
    "a second tab, only when you need two pages at once",
    { url: z.string().optional() },
    "tab_new",
  );
  tool("tab_adopt", "Move one of the user's tabs into your tab group so you can work in it. Only when the user asked you to.", "take one of the user's tabs, only when asked", { tabId: z.number().int() }, "tab_adopt");
  tool("tab_close", "Close one of your tabs.", "close one of your tabs", { tabId }, "tab_close");
  tool(
    "navigate",
    'Go to a URL in your current tab (opening your first tab if you have none), or "back", "forward", "reload". Waits for the page to load.',
    'go to a URL in your tab, or "back", "forward", "reload"',
    { url: z.string(), tabId },
    "navigate",
  );
  tool(
    "read_page",
    "Outline of the page: its interactive elements (or with filter=all, headings and images too), each with a ref to use in browser_computer / browser_form_input, and its position.",
    "the page's elements with refs; filter interactive or all",
    { tabId, filter: z.enum(["interactive", "all"]).optional(), max: z.number().int().optional() },
    "read_page",
  );
  tool("find", "Find elements whose text or label contains all the words of a query. Returns refs.", "elements matching the words of a query, with refs", { query: z.string(), tabId }, "find");
  tool(
    "page_text",
    "The page's visible text, for reading articles, results or tables.",
    "the page's visible text, max characters default 40000",
    { tabId, max: z.number().int().optional().describe("Character limit, default 40000") },
    "page_text",
  );
  tool(
    "form_input",
    "Set a form field (text, textarea, select, checkbox) by ref, the way typing would.",
    "set a form field by ref",
    { ref: z.string(), value: z.union([z.string(), z.boolean(), z.number()]), tabId },
    "form_input",
  );
  tool(
    "computer",
    "Real mouse and keyboard in your tab. Actions: screenshot, left_click, right_click, double_click, triple_click, hover, scroll, type (text), key (e.g. \"Enter\", \"ctrl+a\"), wait (duration seconds). Target with ref (from browser_read_page) or coordinate [x, y] (from a screenshot).",
    "mouse and keys: action screenshot, left_click, right_click, double_click, triple_click, hover, scroll (scroll_direction, scroll_amount), type (text), key (text like Enter or ctrl+a), wait (duration); target a ref or coordinate [x, y]",
    {
      action: z.enum(["screenshot", "left_click", "right_click", "double_click", "triple_click", "hover", "scroll", "type", "key", "wait"]),
      ref: z.string().optional(),
      coordinate: z.array(z.number()).length(2).optional(),
      text: z.string().optional(),
      scroll_direction: z.enum(["up", "down", "left", "right"]).optional(),
      scroll_amount: z.number().optional(),
      duration: z.number().optional(),
      tabId,
    },
    "computer",
  );
  tool("javascript", "Run JavaScript in the page and get the value of its last expression (await allowed).", "run JavaScript in the page, get its last value", { code: z.string(), tabId }, "javascript");
  tool(
    "console",
    "Console messages from your tab since ByteHive started controlling it. Filter with a regex pattern.",
    "console messages, pattern is a regex",
    { pattern: z.string().optional(), onlyErrors: z.boolean().optional(), limit: z.number().int().optional(), clear: z.boolean().optional(), tabId },
    "console",
  );
  tool(
    "network",
    "Network requests from your tab since ByteHive started controlling it. Filter URLs with a regex pattern.",
    "network requests, pattern is a regex on the URL",
    { pattern: z.string().optional(), limit: z.number().int().optional(), clear: z.boolean().optional(), tabId },
    "network",
  );
  tools.push({
    name: "file_upload",
    description: "Attach files from this computer to a file field on the page. ref is the file field, or the button that opens the file chooser. Paths may be relative to the workspace.",
    brief: "attach files to a file field or its button; paths may be relative",
    inputSchema: { ref: z.string(), paths: z.array(z.string()).min(1), tabId },
    run: async (args) => {
      try {
        return await hub.call("upload", { ...args, paths: uploadPaths(args.paths as string[], base) });
      } catch (e) {
        return err(e instanceof Error ? e.message : String(e));
      }
    },
  });
  tool(
    "resize",
    "Resize the Chrome window your tab is in (it is the user's window too; only when a page must be seen at a size, and say so).",
    "resize the user's Chrome window, only when a size matters",
    { width: z.number().int(), height: z.number().int(), tabId },
    "resize",
  );
  tools.push({
    name: "record",
    description: "Record your tab: action start, do the steps, then stop. Saves a video (or format gif) plus timeline.md and keyframes under .bytehive/recordings/ (or path). Read those files only when you need to check something. It shows logged-in pages: ask before sharing.",
    brief: "action start, do the steps, then stop: saves a video (or format gif) under .bytehive/recordings/ or path",
    inputSchema: { action: z.enum(["start", "stop"]), format: z.enum(["video", "gif"]).optional(), path: z.string().optional(), tabId },
    run: async ({ action, format, path: out, tabId: tab }) => {
      const r = await hub.call("record", { action, format, tabId: tab });
      const plain: ToolResult = { content: r.content, ...(r.isError ? { isError: true } : {}) };
      if (action !== "stop" || r.isError) return plain;
      const where = (ext: string) => (typeof out === "string" && out ? (path.isAbsolute(out) ? out : path.resolve(base, out)) : recordingPath(base, agent, ext));
      try {
        if (r.gif) {
          const file = where(".gif");
          fs.mkdirSync(path.dirname(file), { recursive: true });
          fs.writeFileSync(file, Buffer.from(r.gif, "base64"));
          return { content: [...r.content, { type: "text", text: `Saved ${file}` }] };
        }
        if (!r.record) return plain;
        const dir = where("");
        await saveRecording(hub, dir, r.record.files);
        return { content: [...r.content, { type: "text", text: `Saved in ${dir}` }] };
      } catch (e) {
        return err(`The recording was made but not saved: ${e instanceof Error ? e.message : String(e)}`);
      }
    },
  });
  tool("dialog", "Accept or dismiss an alert/confirm/prompt dialog that is blocking your tab.", "accept or dismiss a blocking alert, confirm or prompt", { accept: z.boolean(), text: z.string().optional().describe("Answer for a prompt()"), tabId }, "dialog");
  return tools;
}
