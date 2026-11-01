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
- [x] Usage + giá + limit: `agent-core/usage.rs` (Claude/Codex/opencode),
      `supervise/{pricing,limits}.rs` (commit `5ab7591`). Chưa có: planmeter
      (% gói 5 giờ/tuần), limitguard handover sang agent khác.
- [x] Scheduler: `native-shell/schedule.rs` (port `schedule.ts`).
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
- [x] Spawn: làm theo sheet "New task" của bản đã duyệt (`maestro-inbox.html`
      gộp wizard/spawn modal vào Ctrl+K + New task), không phải wizard grid
      1..12 cũ. Ctrl+N / nút New task / palette "New task", "Race 3 agents":
      prompt, folder, CLI, 1..4 agent (đua), own worktree (tạo ngoài UI
      thread, nhánh trùng thì `-2`), full access; tên persona Ana/Bob...; gõ
      việc vào agent khi tới prompt, chờ khi đang có câu hỏi
      (`supervise/typing.rs` port `typing.ts`). Commit `64977b3`. Bổ sung ở
      `8ad7370`: chế độ quyền (Ask / Accept edits / Bypass), Flow (Just do it /
      Build→Test→Review / Plan first, ghép vào nội dung task), crew đã lưu.
- [x] Workspaces: theo bản đã duyệt là nhiều project mở cùng lúc + chip lọc
      (không phải tab). Chip "All projects / tên · số needs-you", "+" mở thư
      mục (rfd), chuột phải đóng project; lọc grid + Queue, Sessions và New
      task đi theo; lưu ở `~/.maestro/native-projects.json`. Commit `faa6779`.
      Sửa kèm: panel trái egui 0.37 không có max width nên đè mất grid
      (`97709b5`). Bổ sung ở `8ad7370`: màn "Start in a folder" khi chưa mở
      project (project gần đây + crew), nhớ stage theo từng project.
- [x] Settings (Ctrl+,) theo bản đã duyệt: Appearance (cỡ chữ terminal, nền
      4 preset), Agents (CLI/số agent/worktree/quyền/flow mặc định, thời gian
      chờ CLI, crew đã lưu), System (mở lại project). Lưu ở
      `~/.maestro/native-prefs.json`. Chỉ đưa vào setting native thật sự dùng;
      Remote/tray/updates/notify chờ Phase E (cần dashboard, tray, updater
      native). Ảnh nền tùy chọn và theme màu terminal chưa làm. Commit `8ad7370`.

### Phase B: Review code sâu như bản web
Bản đã duyệt ghi "Files open in your editor. Maestro shows the branch; it
doesn't try to be a second IDE", nên Phase B làm theo đó (commit `c7ef337`):
- [x] Files drawer: cây file của nhánh (thư mục trước, đóng sẵn), tìm file,
      dấu A/M/D + chấm trên thư mục có thay đổi, tự đọc lại mỗi 3 giây khi
      mở; chọn nguồn là project hoặc worktree của agent; bấm file mở trong
      VS Code / Cursor (không có thì app mặc định của Windows). Không làm
      multi-select / kéo-thả / context menu của filetree web.
- [x] Editor trong app: KHÔNG làm, theo bản đã duyệt (mở editor của bạn).
- [x] Diff viewer kiểu GitHub (panel Changes): tổng +/-, nhánh → base, tiến độ
      đã xem, danh sách file + ô Viewed, diff có số dòng cũ/mới, phím V; dấu
      đã xem tự bỏ khi file đổi tiếp. File mới (untracked) vào diff dạng toàn
      dòng thêm. Còn thiếu: so sánh đua (Compare race), syntax highlight.

### Phase C: Điều phối fleet + bảng việc
Theo bản đã duyệt ("Board thành view suy ra", "Pomodoro: bỏ"), commit `c338b23`:
- [x] Board (nút Board / Ctrl+K): nửa Agents là 5 cột theo trạng thái thật
      (Free / Working / Needs you / Ready to review / Merged), không kéo thả,
      bấm thẻ đưa agent lên stage. Nửa Plan đọc `.maestro/board.json` của
      project: Approve (Proposed → To do), Move to list bất kỳ, tích
      checklist; ghi an toàn (đọc lại bản mới nhất, giữ trường lạ, temp +
      rename), tự đọc lại mỗi 2 giây. Chưa có: tạo/sửa/xóa thẻ, gán agent.
- [x] Hand-off giữa agent: hiện 3 hand-off gần nhất (30 phút) ngay trong
      Queue; tab Flow giữ lịch sử đầy đủ. Không vẽ connector (canvas bị bỏ).
- [x] Pomodoro + dock: KHÔNG port, bản đã duyệt cắt.

### Phase D: Theo dõi chi phí + lịch
- [x] Chi phí + limit (commit `efed0f1`): chi phí và % context trên header
      pane, chi phí trên thẻ Board, dòng "$X · N agents · M projects" trên
      thanh trên; Queue báo "hit its usage limit, back at 19:00"; tự bảo
      agent làm tiếp 1 phút sau khi limit hết (Settings, mặc định bật).
      Chưa làm: planmeter (% gói), handover khi hết limit, tab History theo
      từng việc (bản đã duyệt muốn gộp cost/replay/lịch vào đó).
- [x] Lịch (commit `0b216c2`): ô Schedule trong New task (giờ + Once/Daily),
      danh sách trong Settings (lần chạy tới, bật/tắt, xóa).

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
