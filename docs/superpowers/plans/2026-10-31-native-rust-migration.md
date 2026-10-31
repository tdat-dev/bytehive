# Maestro: chuyển sang Rust native (plan + trạng thái)

Ngày: 2026-10-31. Nhánh đang làm: `feat/native-shell-r5`.
Mục tiêu: thay bộ đôi Tauri + web (`src-tauri` + `src/`) bằng một app native Rust
(`crates/native-shell`, egui/eframe) dựng trên lõi headless dùng chung, mà giao
diện vẫn đẹp đúng như bản web hiện tại (hệ D.Cinema đã duyệt).

> Trước khi plan này tồn tại, phần Rust đã được dựng thẳng bằng code theo từng
> "vòng" (round 1..5), không có tài liệu. Plan này ghi lại kiến trúc đích, đo
> xem đã xong tới đâu, và liệt kê đủ bước còn lại để hoàn tất.

## 1. Vì sao chuyển

- Bản web trên Tauri/WebView2 kéo theo các lớp lỗi riêng của WebView (HTML5
  drag-drop vỡ, sync command đơ main thread, CDP để chụp màn...). Native egui
  bỏ hẳn tầng WebView.
- Lõi logic (parse transcript, phát hiện ask/stuck/clash, diff, done-check) vốn
  nặng và đã muốn tách khỏi UI. Chuyển Rust cho phép UI gọi thẳng lõi, không qua
  IPC `invoke`.
- Một binary native, khởi động nhanh, một hệ màu/khoảng cách thống nhất.

## 2. Kiến trúc đích (3 crate + lõi dùng chung)

```
crates/agent-core   headless: spawn PTY, registry vòng đời pane, launch spec,
                    đọc session/transcript từng CLI (claude/codex/muse/opencode),
                    git + worktree. KHÔNG biết gì về UI.
crates/supervise    bộ não: parse ask + chấm rủi ro (ask, ask_risk), phát hiện
                    clash, stuck, task_state; describe/digest/done_check;
                    diff_file, review_text, screens, ledgers, text, types;
                    chat-model từng CLI. Thuần logic, test được không cần cửa sổ.
crates/native-shell UI egui: đọc thẳng agent-core + supervise (không IPC). Mỗi
                    surface một module, mọi màu/size đọc từ theme.rs.
```

Đích cuối: `native-shell` thay vai trò của `src-tauri` (backend lệnh) lẫn `src/`
(UI web). MCP server (`mcp/`) và remote dashboard giữ là tiến trình riêng do
native spawn/quản lý, không nhét vào UI.

## 3. Đã hoàn thành tới đâu

### 3a. Lõi headless: gần như XONG

`agent-core` và `supervise` đã mirror phần lớn model của bản web:

- [x] Spawn/PTY/registry vòng đời pane: `agent-core/{registry,pty_session,pipe_session,launch,agent}.rs`
- [x] Đọc session + transcript 4 CLI: `claude/codex/muse/opencode.rs` (cả 2 crate)
- [x] Parse câu hỏi của agent + chấm rủi ro: `supervise/{ask,ask_risk}.rs` (web `askparse`)
- [x] Phát hiện clash giữa agent: `supervise/clash.rs` (web `clashwatch`)
- [x] Phát hiện kẹt/stuck: `supervise/stuck.rs` (web `stuckwatch`)
- [x] Trạng thái việc (working/needs-you/review...): `supervise/task_state.rs` (web `taskstate`)
- [x] Mô tả / tóm tắt / done-check: `supervise/{describe,digest,done_check,review_text}.rs`
- [x] Diff + screens + ledgers + text + types: `supervise/*`
- [x] Git + worktree: `agent-core/git.rs`

Còn thiếu ở lõi (khi UI cần tới mới port nốt):
- [ ] Theo dõi usage/limit theo gói (web `limits`, `planmeter`, `agentcost`,
      `limitguard`): mới có `ledgers.rs`, chưa đủ parity.
- [ ] Scheduler / lịch chạy agent (web `scheduler`, `schedule`).
- [ ] Project-memory + MCP memory (web `projectmemory`, `memoryview`).

### 3b. UI native-shell: ĐANG DỞ (ước tính ~45% bề mặt)

Đã dựng và có test (62+ test xanh, length gate ok):

- [x] Khung: top bar spawn, panel trái 5 tab, grid panes, theme + Geist fonts
      (`theme.rs` port 1:1 từ `src/styles/tokens.css`).
- [x] Tab Queue: xếp hạng needs-you, thẻ attention, trả lời nhanh inline.
- [x] Tab Review: diff +A/-B, merge nhánh worktree về, checkpoint/commit.
- [x] Tab History: tail transcript live (throttle), retry đọc rỗng.
- [x] Tab Sessions: resume phiên cũ + form spawn trong worktree mới.
- [x] Tab Flow: tail fleet outbox (hand-off giữa agent).
- [x] Send box: gửi text tới bất kỳ pane đang chạy.
- [x] Command palette Ctrl+K: nhảy agent + chạy lệnh (New Shell / Spawn <CLI> /
      Go to <tab>); Ctrl+1..9 nhảy nhanh theo thứ hạng Queue. (commit `4594ebb`)
- [x] Terminal: render bằng alacritty_terminal + vte, copy OSC52, density zoom
      Ctrl + / - / 0.
- [x] Focus stage + rail avatar, grid tileToFit/nextSlot (port từ canvas web).
- [x] Spawn: fresh boot từng CLI, resume, worktree.

## 4. Giữ giao diện đẹp (track design, chạy song song mọi phase)

Chuẩn thẩm mỹ: bản `design/redesign/maestro-inbox.html` đã duyệt (look D.Cinema),
cùng `src/styles/tokens.css`. Nguyên tắc: không chế UI generic, không template,
không Inter/Roboto mặc định, chỉ dùng màu cho state.

- [x] Nền near-black, 4 tầng surface, text 3 tier, màu state (green/blue/amber/red)
      đã vào `theme.rs`; Geist + Geist Mono vendored và cài làm family mặc định.
- [ ] Gỡ `#![allow(dead_code)]` trong `theme.rs` khi các token "reserved cho
      Native 2/3" đã có surface dùng.
- [ ] Mỗi surface mới PHẢI đi qua skill design (`frontend-design` /
      `impeccable`) đối chiếu `maestro-inbox.html` trước khi dựng, rồi review
      bằng screenshot (1440 + kích thước hẹp) như bản web.
- [ ] Nền canvas chỉnh được (preset/màu/ảnh) như board P3 web, lưu per-workspace.
- [ ] Motion thống nhất, tinh tế (chỉ dịch y/scale, không opacity:0 trên cả vùng).

## 5. Các bước còn lại để chuyển ĐỦ (phase hóa)

Thứ tự theo giá trị dùng hằng ngày và phụ thuộc.

### Phase A: Spawn + workspace cho dùng thật được
- [ ] Spawn wizard đầy đủ (web `spawnmodal`): chọn số lượng, grid 1..12, chọn
      nhiều model một lần, preset; thay cụm nút `+ CLI` hiện tại.
- [ ] Workspaces nhiều tab (web `workspace`, `workspaces`): mở nhiều thư mục
      dự án, nhớ trạng thái per-workspace.
- [ ] Settings/Preferences đầy màn (web `settingsmodal`, `prefs`, `prefsview`,
      `termtheme`, `ansibg`): Appearance / Fleet / Sessions / System + background
      picker; mọi thứ hiện có trong app web.

### Phase B: Review code sâu như bản web
- [ ] File explorer / tree (web `filetree`): multi-select, phím, context menu,
      kéo-thả bằng Pointer-equivalent của egui, watcher realtime (`core::watch`).
- [ ] Editor / code panel (web `editor`, `codepanel`): mở sửa file, đồng bộ khi
      agent đổi file trên đĩa.
- [ ] Diff viewer đầy đủ (web `diffview`): hơn mức diff gọn trong `review_ui`.

### Phase C: Điều phối fleet + bảng việc
- [ ] Kanban board trong app (web `kanban`, `board`, `boardfile`): đọc/sửa
      `.maestro/board.json`, kéo thẻ giữa list, gán agent.
- [ ] Delegation connectors + crew/fleet (web `delegation`, `crew`, `fleet`,
      `fleetbridge`): vẽ khi agent giao việc nhau; Flow tab đã là mầm.
- [ ] Tool dock: Pomodoro + dock (web `pomodoro`, `dock`, `dockstore`).

### Phase D: Theo dõi chi phí + lịch
- [ ] Usage/limit/cost UI (web `usage`, `limits`, `pricing`, `planmeter`,
      `agentcost`, `limitguard`) + hoàn tất parity lõi `ledgers.rs`.
- [ ] Scheduler / routines (web `scheduler`, `schedule`).

### Phase E: Trình duyệt + remote + hệ thống
- [ ] Maestro Browser trong native (web `browserview`, `browserprefs`,
      `bridges`): xem screencast, Dừng / Tôi làm tiếp (xem thẻ To-do board +
      spec `2026-09-24-maestro-browser-design.md`).
- [ ] Remote fleet dashboard native-side (web `dashboard`, `dashpw`,
      `dashorder`): các lệnh backend đã có trong `src-tauri/dashboard`, cần UI +
      gate login (xem nhóm thẻ Login trên board) và password.
- [ ] Hệ thống: tray + quit-life + power (web `quitlife`, `power`), updater
      (web `updater`), titlebar, intro, tips/hint/calm.
- [ ] Project-memory + memory view (web `projectmemory`, `memoryview`).
- [ ] Replay + handoff + convdigest (web `replay`, `handoff`, `convdigest`).

### Phase F: Thay thế + đóng gói + rút lui Tauri
- [ ] Parity backend: mọi lệnh `src-tauri` cần thiết đã có đường native tương
      đương (hiện `src-tauri` đăng ký ~90 command trong `lib.rs`).
- [ ] Đóng gói native binary thành installer; đẩy artifact + `latest.json` lên
      public repo `tdat-dev/maestro-releases` (giữ pipeline release hiện tại).
- [ ] Giữ MCP server (`mcp/`) + dashboard là tiến trình riêng do native quản lý.
- [ ] Khi parity đủ: gỡ `src/` + `src-tauri`, đổi entry chính sang native, bỏ
      "(prototype)" khỏi tiêu đề cửa sổ (`crates/native-shell/src/main.rs`).

## 6. Quyết định còn mở (hỏi user trước khi làm phase liên quan)

1. Bỏ hẳn free-canvas kéo-thả + detach-ra-cửa-sổ (web `canvas`, `windetach`,
   `panelayout`) hay giữ? Hiện native dùng grid + focus-stage/rail, đơn giản hơn.
2. Agent Inbox (bản redesign đã duyệt) map vào native thành 5 tab hiện tại, hay
   dựng lại đúng layout `maestro-inbox.html`?
3. Voice dispatch (board P2) có nằm trong phạm vi native không?
4. Tính năng web nào cắt hẳn thay vì port (để giảm diện tích)?

## 7. Định nghĩa "XONG"

- Mọi surface dùng hằng ngày của bản web có đường tương đương trong native.
- Hệ màu/typography/motion khớp bản đã duyệt; qua review screenshot.
- `cargo test` xanh toàn bộ; `npm run check:length` ok; không file > 500 dòng.
- App native đóng gói, cài được, thay entry chính; `src/` + `src-tauri` gỡ đi.
- Nhịp commit theo từng phần việc (checkpoint), release khi tới mốc dùng được.

## Tham chiếu

- Thiết kế đã duyệt: `design/redesign/maestro-inbox.html`, `src/styles/tokens.css`.
- Spec liên quan: `docs/superpowers/specs/2026-09-24-maestro-browser-design.md`,
  `docs/superpowers/specs/2026-07-21-maestro-canvas-redesign.md`.
- Lõi: `crates/agent-core/src/`, `crates/supervise/src/`.
- UI: `crates/native-shell/src/` (app.rs, grid.rs, *_ui.rs, palette.rs, term_pane.rs).
