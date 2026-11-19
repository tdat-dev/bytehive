# Browser video recording (browser_record)

Date: 2026-11-19. Owner asked for fuller video recording from "ByteHive for Chrome" so agents need computer use less.

## Goals

- An agent records its own tab as a real video (MP4, WebM when Chrome cannot write MP4), smooth, at the page's own paint rate up to about 30 fps.
- The same recording serves two readers: the owner watches the video; the agent reads a short timeline and, only when it needs to, a keyframe image.
- No click from the owner: works on the agent's tab even when it is covered or behind another window.
- The video looks like a person using a normal Chrome: a plain white arrow, no agent name, no colour frame, no "is using this tab" pill, no typed-text chip, no coloured click rings.
- Cheap in tokens (owner rule 2026-11-19): no new tool (browser_record replaces browser_gif), a short tool description, and a stop result of a few text lines. Images and the timeline go to files only.

## Not doing

- Sound. It needs chrome.tabCapture, which Chrome allows only after the user clicks the extension on that tab each time.
- Whole screen or other app windows (needs Chrome's picker every time).
- Several tabs in one video.

## How it works

1. `browser_record start` (MCP) calls hub tool `record` with `{action:"start", format}`.
2. The extension (service worker) attaches the debugger as it already does, sets the cursor to plain mode, and runs `Page.startScreencast` (JPEG, quality 80, at most 1920x1080). Each `Page.screencastFrame` is acked at once and passed to an offscreen document (`record/offscreen.html`).
3. The offscreen document draws each frame on a canvas whose size is fixed by the first frame (even numbers, for H.264); later frames of another size are fitted with black bars. `canvas.captureStream(0)` plus `requestFrame()` per frame, and once every 500 ms while the page is still, feeds a `MediaRecorder` (`video/mp4;codecs=avc1` first, then VP9 WebM, 4 Mbit/s, 1 s slices).
4. While recording, the service worker writes a timeline: every agent action on that tab (navigate, click with point and ref, type, key, scroll, form input, upload), console errors and exceptions, and failed requests (status 400+ or a network error), each with its second in the video. The same lines repeated are folded into one with a count.
5. Keyframes: the newest screencast frame is kept; at each action it is saved as `before`, and the first frame at least 700 ms after the action as `after`. Also the first and last frames. Same image twice is kept once. At most 60.
6. `stop`: screencast ends, the cursor leaves plain mode, the recorder finishes. The result holds the summary text and a list of files with sizes. The MCP side then pulls each file in parts of 4 MB (`{action:"part", name, offset}`) and ends with `{action:"done"}`, which frees the memory. Parts keep every hub message far below tungstenite's 16 MB frame and Chrome's limits. An unclaimed finished recording is dropped after 10 minutes.
7. A recording stops by itself after 10 minutes; `stop` then returns what was recorded.

`format:"gif"` keeps the old GIF path (captureVisibleTab twice a second), moved into `record/gif.js` unchanged. The hub tool `gif` stays in the extension so an older bytehive-mcp keeps working.

## Files written

`<workspace>/.bytehive/recordings/<agent>-<time>/` (or the `path` given):

- `video.mp4` (or `video.webm`)
- `timeline.md`: one line per event, `0:12.4 click "Save" (412,188)`
- `frames/NNN-<second>s-<before|after>-<action>.jpg`

The stop result is only: the folder, video length and size, the count of actions, errors and frames, and "read timeline.md or a frame only when you need it".

## Cursor while recording

`cursor.js` gets a `plain` flag. With it: the arrow is drawn, glides and presses as before; the name tag, chip, frame, pill and click rings are not drawn. The pill's Stop button is hidden too, so during a recording the owner stops an agent from the ByteHive app.

## Code layout

`background.js` is already 749 lines, so recording leaves it:

- `record/index.js`: the `record` and `gif` tool calls, the per-agent state, noting actions.
- `record/screencast.js`: CDP screencast on and off, frames to the offscreen document, keyframes.
- `record/timeline.js`: pure functions for events, folding and markdown (unit tested with node).
- `record/offscreen.html`, `record/offscreen.js`: canvas, MediaRecorder, parts.
- `record/gif.js`: the old GIF code.

`manifest.json` adds the `offscreen` permission. In MCP, `browser_gif` becomes `browser_record` (`action`, `format`, `path`, `tabId`).

## Errors

- A tab Chrome will not let the extension drive (chrome://, Web Store): start fails with the existing message.
- No frame came (the tab never painted): stop says so and writes no video.
- The tab closes while recording: the recording stops and keeps what it has.
- `start` while recording: the old recording is stopped and dropped first.

## Testing

- `node --test` on `record/timeline.js`.
- MCP vitest: the tool list has `browser_record`, not `browser_gif`; the saving of parts into files.
- A real run in Chrome with the extension: record a few clicks on a page, check the MP4 plays, has the plain cursor and no overlays, and the timeline and frames are right.
