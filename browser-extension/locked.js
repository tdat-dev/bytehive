// Pages Chrome keeps from every extension: chrome:// pages, the Web Store,
// other extensions' pages. No script, no debugger, no screenshot reaches them
// from here ("cannot be scripted"), so the step goes to the ByteHive app, which
// reads and drives the window from Windows the way a screen reader does
// (native/src/desktop). This side only brings the tab and its window to the
// front and says which window it is.

const OWN = chrome.runtime.id;

/** Tools that touch the page itself, which Chrome blocks here. */
export const PAGE_TOOLS = new Set(["read_page", "find", "page_text", "form_input", "computer", "upload", "javascript", "console", "network", "dialog"]);

export function locked(url) {
  if (!url) return false;
  if (/^(chrome|chrome-untrusted|chrome-search|devtools|edge|brave|opera|vivaldi|view-source):/i.test(url)) return true;
  if (/^chrome-extension:\/\//i.test(url)) return !url.startsWith(`chrome-extension://${OWN}/`);
  return /^https:\/\/(chromewebstore\.google\.com|chrome\.google\.com\/webstore|microsoftedge\.microsoft\.com\/addons)(\/|$)/i.test(url);
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/** Bring the tab forward and hand the step to ByteHive on Windows. */
export async function toDesktop(t) {
  await chrome.tabs.update(t.id, { active: true });
  const w0 = await chrome.windows.get(t.windowId);
  await chrome.windows.update(t.windowId, w0.state === "minimized" ? { focused: true, state: "normal" } : { focused: true });
  await sleep(250);
  const tab = await chrome.tabs.get(t.id);
  const w = await chrome.windows.get(t.windowId);
  return {
    content: [{ type: "text", text: "This page is driven from Windows by ByteHive." }],
    desktop: {
      tabId: tab.id,
      windowId: tab.windowId,
      title: tab.title ?? "",
      url: tab.url ?? tab.pendingUrl ?? "",
      bounds: { left: w.left, top: w.top, width: w.width, height: w.height },
    },
  };
}
