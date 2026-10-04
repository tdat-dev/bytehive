# Remote fleet dashboard

Settings, Remote, *Remote fleet dashboard* starts a small web page (port 8477)
that shows your agents and lets you type into them from a phone's browser.

## Who can reach it

- **Off by default.** When on, it listens on this computer only
  (`http://127.0.0.1:8477`).
- **Other devices** (the *Allow other devices (LAN)* toggle) need a password
  first. Anyone opening the dashboard then signs in with it, this computer
  included. A password is at least 10 characters.
- Removing the password stops the LAN server before the password goes, and
  signs everyone out.

## On your Wi-Fi the password goes unencrypted

The dashboard speaks plain `http`, not `https`. On the LAN, the password you
type and the session cookie that keeps you signed in cross the network as is:
anyone who can watch traffic on that network can read them and then drive your
agents. Use the LAN toggle only on a network you trust (your home Wi-Fi), never
on a cafe, hotel, office guest or other shared network. On this computer
(`127.0.0.1`) nothing leaves the machine.

## What it refuses

- Requests whose `Host` is not `127.0.0.1:8477`, `localhost:8477` or the LAN
  address shown in Settings (stops DNS rebinding pages).
- Requests with an `Origin` other than the dashboard page itself (stops other
  sites, and other servers on this computer such as a dev server on port 3000).
- POSTs that are not `Content-Type: application/json`, and API calls without
  the `X-Maestro: 1` header the page sends. Another site cannot send either
  without a CORS preflight, which the dashboard never answers.
- Sign-in bursts: at most two password checks run at once (the rest get
  "Too many attempts, try again in a moment"), an address that keeps guessing
  waits longer each time, and all addresses together get 10 misses a minute.

Signing out, changing or removing the password, or a session running out
(12 hours) closes any terminal that is open on the phone within a second.
