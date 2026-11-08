# Releasing ByteHive (auto-update)

ByteHive is the native app in `crates/native-shell`. An installed copy checks
`https://github.com/tdat-dev/bytehive-releases/releases/latest/download/latest.json`
a little after start and every six hours (Settings > System), and when a newer
**signed** build exists the top bar offers "Update to x.y.z". Installing
downloads the installer, checks its minisign signature against the key built
into `crates/native-shell/src/updater.rs`, runs it, and closes ByteHive; the
installer opens it again.

The source repo `tdat-dev/bytehive` is **private**. Installers and `latest.json`
go to the **public** `tdat-dev/bytehive-releases` (releases only, no code).

It was named `tdat-dev/maestro-releases` until 2026-11-08. Copies installed before
then still ask that address and GitHub redirects them here, so **never create a
repo named `maestro-releases`**: it would take the redirect and strand them.

## Moving people off the Tauri app

The Tauri app (0.10.x and earlier) reads the same `latest.json` with the same
key. The native installer installs where the Tauri app was, under its uninstall
key `bytehive`, and accepts the Tauri updater's `/P` (passive) flag, and every
Windows key in `latest.json` (`windows-x86_64`, `-nsis`, `-msi`) points at the
NSIS installer. So the first native release reaches Tauri users through their
own updater, and they end up with one ByteHive.

## Cutting a release

1. Bump `version` in `crates/native-shell/Cargo.toml` (and `package.json`).
2. Add a `## [x.y.z]` section to `CHANGELOG.md`: its text becomes the release
   notes and what the update button shows.
3. Commit, then tag and push: `git tag vx.y.z && git push origin vx.y.z`.

The **Release** workflow (`.github/workflows/release.yml`) then:

- checks the tag matches the crate's version,
- builds the installer (`tools/build_installer.ps1`: `cargo build --release`
  and NSIS with `crates/native-shell/installer/bytehive.nsi`),
- signs it with `npx @tauri-apps/cli signer sign` (secrets
  `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`),
- writes `latest.json` (`tools/latest_json.mjs`),
- publishes the installer, its `.sig` and `latest.json` to
  tdat-dev/bytehive-releases with `RELEASES_TOKEN`.

## Building the installer by hand

```powershell
npm run installer   # crates\native-shell\target\release\bundle\ByteHive_<v>_x64-setup.exe
```

Needs NSIS 3 (`makensis` on PATH, in Program Files, or the copy Tauri left in
`%LOCALAPPDATA%\tauri\NSIS`).
