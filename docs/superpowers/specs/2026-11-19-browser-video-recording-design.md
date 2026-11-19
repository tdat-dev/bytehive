# Browser video recording (browser_record)

Date: 2026-11-19. Owner asked for fuller video recording from "ByteHive for Chrome" so agents need computer use less.

## Goals

- An agent records its own tab as a real video: MP4 at a steady 30 frames a second, with as many new pictures as the tab paints (about 30 a second measured when the machine had room; fewer on a machine already at full load, where a picture is held over several frames).
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
2. The extension (service worker) attaches the debugger as it already does, sets the cursor to plain mode, and runs `Page.startScreencast` (JPEG, quality 70, at most 1920x1080). Each `Page.screencastFrame` goes to an offscreen document (`record/offscreen.html`) with the time Chrome painted it (moved onto the worker's clock by the gap seen on the first frame). Up to 8 pictures may be on their way at once; past that the ack waits, so Chrome slows down instead of piling pictures up.
3. The offscreen document only decodes and queues a picture (one per 1/30 s slot: a newer one in the same slot replaces the older), then answers. A separate loop encodes a steady 30 frames a second, 250 ms behind real time, each tick with the newest picture painted by then, using WebCodecs `VideoEncoder` (H.264 High, software encoder first, then any; VP9 when there is no H.264; realtime mode, 2.5 Mbit/s variable) and `vendor/mp4-muxer.mjs` (MIT). Each picture becomes one `VideoFrame` (straight from the bitmap when it is the video's size or a pixel more), and each tick encodes a copy of it with the tick's time, so a repeated picture moves no pixels. Time 0 of the video is the start of the recording, the same as the timeline's. The size is the tab's view in device pixels, measured 1.5 s after the debugger attached (Chrome's "started debugging" bar shortens the view a moment after). Stop encodes every tick up to the stop time and flushes the encoder.
   - Measured, not guessed: a canvas stream with `MediaRecorder` in the hidden offscreen document gave 3 fps or an empty file (Chrome takes a canvas frame only when it renders that page). Encoding inside the picture's own call made Chrome wait and sent 2 pictures a second. On a busy machine the GPU encoder (Media Foundation) fell 9 s behind a 30 fps clock, the software one stayed within 0.5 s. With one picture in flight the tab sent 13 a second; with 8, about 30 when the machine had room.
   - A covered window paints only while something animates, so a still page or a change made by script can send no screencast frame. After a second without one, the service worker takes `Page.captureScreenshot` (which makes Chrome paint) as the next frame.
4. While recording, the service worker writes a timeline: every agent action on that tab (navigate, click with point and ref, type, key, scroll, form input, upload), console errors and exceptions, and failed requests (status 400+ or a network error), each with its second in the video. The same lines repeated are folded into one with a count.
5. Keyframes: the newest screencast frame is kept; at each action it is saved as `before`, and the first frame at least 700 ms after the action as `after`. Also the first and last frames. Same image twice is kept once. At most 60.
6. `stop`: screencast ends, the cursor leaves plain mode, the recorder finishes. The result holds the summary text and a list of files with sizes. The MCP side then pulls each file in parts of 4 MB (`{action:"part", name, offset}`) and ends with `{action:"done"}`, which frees the memory. Parts keep every hub message far below tungstenite's 16 MB frame and Chrome's limits. An unclaimed finished recording is dropped after 10 minutes.
7. A recording stops by itself after 10 minutes; `stop` then returns what was recorded.

`format:"gif"` keeps the old GIF path (captureVisibleTab twice a second), moved into `record/gif.js` unchanged. The hub tool `gif` stays in the extension so an older bytehive-mcp keeps working.

## Files written

`<workspace>/.bytehive/recordings/<agent>-<time>/` (or the `path` given):

- `video.mp4`
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
- `record/offscreen.html`, `record/offscreen.js`: VideoEncoder, MP4 muxer, parts.
- `record/gif.js`: the old GIF code.

`manifest.json` adds the `offscreen` permission. In MCP, `browser_gif` becomes `browser_record` (`action`, `format`, `path`, `tabId`).

## Errors

- A tab Chrome will not let the extension drive (chrome://, Web Store): start fails with the existing message.
- No frame came: stop takes one screenshot first, so even a page that never painted gets a short video.
- The console replays messages when a page comes back from the back/forward cache: the extension drops a message it has already seen (same time, kind and text), for browser_console too.
- The tab closes while recording: the recording stops and keeps what it has.
- `start` while recording: the old recording is stopped and dropped first.

## Testing

- `node --test` on `record/timeline.js`.
- MCP vitest: the tool list has `browser_record`, not `browser_gif`; the saving of parts into files.
- A real run in Chrome with the extension: record a few clicks on a page, check the MP4 plays, has the plain cursor and no overlays, and the timeline and frames are right.
