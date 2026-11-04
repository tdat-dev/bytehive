// Draw a list of HTML blocks into an element, keeping the nodes of blocks
// that didn't change. The chat view used to replace its whole thread (up to
// 160 messages, their markdown, their pictures) on every step an agent took;
// now a new step adds one node and a step that finished swaps only its own.
// Nodes kept keep everything live on them: focus, scroll inside, find marks.

/** One element to draw. With `kids`, `html` is its empty shell and the kids
 *  are drawn inside it the same way (a group of steps). */
export interface Block {
  key: string;
  html: string;
  kids?: Block[];
}

interface Drawn { html: string; el: Element }
const drawnIn = new WeakMap<Element, Map<string, Drawn>>();

function elementOf(html: string): Element {
  const t = document.createElement("template");
  t.innerHTML = html;
  if (t.content.childNodes.length === 1 && t.content.firstElementChild) return t.content.firstElementChild;
  // not a single element: keep it together in a box that adds no layout
  const box = document.createElement("div");
  box.style.display = "contents";
  box.append(t.content);
  return box;
}

/** Make `host`'s children the blocks, in order: a block drawn before with the
 *  same key and the same html keeps its node, anything else is drawn anew, and
 *  what is no longer there goes. */
export function drawBlocks(host: Element, blocks: Block[]): void {
  const prev = drawnIn.get(host) ?? new Map<string, Drawn>();
  const next = new Map<string, Drawn>();
  let cursor: ChildNode | null = host.firstChild;
  for (const b of blocks) {
    if (!b.html) continue;
    let key = b.key;
    for (let n = 2; next.has(key); n += 1) key = `${b.key}#${n}`;
    let d = prev.get(key);
    if (!d || d.html !== b.html || d.el.parentNode !== host) d = { html: b.html, el: elementOf(b.html) };
    next.set(key, d);
    if (b.kids) drawBlocks(d.el, b.kids);
    if (d.el === cursor) cursor = cursor.nextSibling;
    else host.insertBefore(d.el, cursor);
  }
  // Whatever is left after the last block is no longer in the list.
  while (cursor) {
    const n: ChildNode | null = cursor.nextSibling;
    cursor.remove();
    cursor = n;
  }
  drawnIn.set(host, next);
}

/** Forget what was drawn in `host` (its next draw builds everything anew). */
export function forgetBlocks(host: Element): void {
  drawnIn.delete(host);
}
