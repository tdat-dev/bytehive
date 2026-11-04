# ByteHive for Chrome: privacy policy

Last updated: 2026-09-25

ByteHive for Chrome connects your browser to **ByteHive**, a desktop app you
install on your own computer. It lets the AI agents you run in ByteHive use your
browser: open tabs, read pages, click and type. This page says what the
extension sees and where it goes.

## What the extension handles

When one of your agents asks it to act, the extension reads the page it acts
on: the page's text and the list of buttons and fields, screenshots of the tab,
and, when the agent asks for them, the tab's console messages and network
requests. When you start a recording, it takes pictures of the tab to make a
GIF. When an agent attaches a file, Chrome reads that file from your disk.

It also reads the email address of the Chrome profile it runs in, so ByteHive
can show you which profile is connected.

## Where it goes

Everything goes to the ByteHive app **on the same computer**, through Chrome's
native messaging. The extension has no server of its own, sends nothing to us
or to anyone else, and contains no analytics or tracking.

ByteHive passes what an agent asked for to that agent. The agents are programs
you choose and run yourself (for example Claude Code or Codex), under your own
accounts with their providers, and they may send what they read to those
providers as part of their work. Their own privacy policies apply to that.

GIF recordings are saved as files on your computer, in the folder you work in.

## What it keeps

The extension keeps only which tab group belongs to which agent, for as long
as the browser session lasts. It keeps no browsing history, no page content and
no screenshots.

## Your control

- The extension acts only when an agent in ByteHive asks it to. Each agent works
  in its own tab group, named after it, and can't touch your other tabs unless
  you hand one over.
- A frame and a "… is using this tab" pill show while an agent works; Stop on
  that pill, or in ByteHive, stops it.
- Clicks that send, post, pay or delete wait for your OK in ByteHive.
- On a login page or a CAPTCHA the agent is told to stop and let you handle it.
- Removing the extension, or closing ByteHive, ends all of this.

## Contact

Questions: open an issue at https://github.com/tdat-dev/maestro-releases/issues.
