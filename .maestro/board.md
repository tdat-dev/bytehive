# Board

## Proposed (24)

- [ ] Login: Gate dashboard web (Rust)
  Bắt session cookie cho dashboard server; đóng lỗ hổng /api/send gõ lệnh vào agent không cần auth.
  - [ ] Session store in-memory trong Dashboard state: token 32-byte CSPRNG + hạn 12h
  - [ ] POST /api/login: verify qua auth, set Set-Cookie maestro_sess HttpOnly SameSite=Strict
  - [ ] POST /api/logout: xóa session
  - [ ] Chặn /api/fleet & /api/send: thiếu cookie hợp lệ = 401
  - [ ] GET / chưa auth → phục vụ view login trong dashboard.html
  - [ ] Backend từ chối login khi chưa cấu hình credential
  - [ ] Test: 401 khi thiếu cookie, 200 khi có; login đúng/sai; session hết hạn
- [ ] Login: Khóa app desktop + boot gate (frontend)
  Overlay lock/setup theo phong cách app; nhớ máy này, có nút Khóa thủ công. Agent chạy nền phía dưới.
  - [ ] ipc.ts: wrapper cho auth_status/setup/verify/lock/change
  - [ ] src/auth.ts: quyết định boot (setup vs lock vs pass-through)
  - [ ] src/styles/auth.css: overlay tối, font Geist, accent #c6f135, dấu Maestro
  - [ ] Markup overlay trong index.html (setup + login panel)
  - [ ] main.ts: gọi auth_status lúc boot, chặn Home đến khi mở khóa
  - [ ] Nhớ máy: flag maestro.unlocked (localStorage); relaunch không hỏi lại
  - [ ] auth.ts test: 3 nhánh boot với ipc mock
- [ ] Login: Settings, first-run & spec/docs
  Nút Khóa + đổi đăng nhập trong Settings; disable toggle dashboard đến khi có credential; ghi spec + đường dẫn khôi phục.
  - [ ] Settings: nút 'Khóa ngay' → auth_lock + hiện màn đăng nhập
  - [ ] Settings: đổi username/password (auth_change)
  - [ ] Disable toggle 'Remote fleet dashboard' đến khi đã tạo login (tooltip nhắc)
  - [ ] Ghi spec docs/superpowers/specs/2026-07-11-login-design.md
  - [ ] Ghi chú khôi phục: xóa auth.json trong app-config dir → về first-run
  - [ ] Cập nhật cảnh báo LAN: đã có mật khẩu chắn
- [ ] Redesign P3: Full settings + backgrounds + English chrome
  Settings full màn 4 mục (Appearance/Fleet/Sessions/System) gộp toàn bộ setting hiện có; đổi nền canvas (preset+màu+ảnh); topbar gọn; toàn app English.
  - [ ] Settings full-screen: sidebar 4 mục, gom mọi setting Maestro hiện có + background picker
  - [ ] Background: preset/màu/ảnh (data-URI) áp vào canvas, lưu per-workspace
  - [ ] Topbar gọn (logo·tabs·live); command bar chrome; token/CSS dùng chung
  - [ ] i18n: chuyển toàn bộ UI sang tiếng Anh
  - [ ] Verify: mọi màn render đúng light? (dark-only) + screenshot review
- [ ] Tin gửi giữa các agent bị mất khúc giữa
  ĐÃ TÌM RA + ĐÃ SỬA (commit 636a92b). Đo trên pane Claude Code thật: tin 200 dòng/10k ký tự gõ kiểu chunk chỉ tới 93 dòng, mất theo khối 21/21/41/41/21 dòng (~1KB và ~2KB), có dòng bị nối giữa chừng. Probe ghi stdin thô nhận đủ 10.000/10.000 byte -> đường truyền sạch, lỗi nằm ở TUI đoán ranh giới paste. Bọc bracketed paste: 200/200, không mất dòng nào.
  - [ ] Kiểm chứng trên fleet thật sau khi rebuild app (chưa làm — cần restart)
  - [ ] Cân nhắc file-pointer cho tin rất dài nếu bracketed paste vẫn hụt ở kích thước lớn hơn 10k
  - [ ] Thử lại với pane Codex để xác nhận Codex cũng bật DECSET 2004
- [ ] Fleet Console — chat trượt lên thay command bar
  STALE 2026-10-27: built in e5b98c6 then intentionally removed in 5ca58d7 (bottom composer gone by design; type into agent terminals, hand-offs stay in Flow panel). Reopen only if that decision is reversed. --- Thay hẳn thanh 1 dòng bằng console đáy: gõ/bấm → trượt lên thành luồng hội thoại (bubbles từ Flow + broadcast + needs-you), composer xịn giữ nguyên send/target/mic. Ref Mobbin: Campsite/Teams/Base44.
  - [ ] index.html: bọc composer trong .fleet-console; thêm .fc-thread (log bong bóng) + header thành viên + nút lịch sử; giữ nguyên id composer (bcastInput/Send/Target)
  - [ ] src/console.ts: render thread từ flowLog() + onFlow live; bubble mine(phải)/others(trái+avatar)/system(needs-you); header thành viên + status; click bubble → revealPane; mở/đóng + unread
  - [ ] broadcast.ts: đẩy tin bạn gửi vào flow buffer (You→target) để hiện trong thread (và voice dispatch)
  - [ ] styles/console.css: container đáy, thread slide-up, bubbles, members strip, composer; z-index; reduced-motion; contrast ≥ 4.5:1
  - [ ] Hành vi: focus/nút mở thread, Esc/click-away đóng, unread badge khi đóng, auto-scroll đáy
  - [ ] Verify: tsc + build + full tests xanh; live
- [ ] Lập danh mục chức năng
  Liệt kê toàn bộ feature từ README + src/ modules (wizard, terminal, broadcast, diff, kanban, pomodoro, dock, mascot)
- [ ] Phân tích Workspace & Wizard
  Đọc wizard.ts + workspaces.ts: luồng chọn folder, preset, grid 1-12, multi-select model
- [ ] Phân tích Terminal/ConPTY
  Đọc terminal.ts + ipc.ts + src-tauri: spawn agent, tree-kill, xterm.js, vòng đời pane
- [ ] Phân tích Tiling & Detach/Merge
  Trong main.ts: tiling frameless, control pill, drag reorder, detach ra cửa sổ riêng rồi merge
- [ ] Phân tích Diff review
  Đọc diff.ts + diffview.ts: cách review thay đổi từng agent, hiển thị diff
- [ ] Phân tích Tool dock (Kanban/Pomodoro)
  Đọc dock.ts, dockstore.ts, kanban.ts, pomodoro.ts: tính năng v0.2.0 mới thêm
- [ ] Phân tích Broadcast & Crew
  Đọc crew.ts: broadcast 1 lệnh tới nhiều agent, chọn nhóm agent nhận
- [ ] Đánh giá chất lượng & độ phủ test
  Soát các *.test.ts hiện có, chỉ ra module thiếu test và rủi ro
- [ ] Tổng hợp báo cáo phân tích
  Viết docs/feature-analysis.md: bảng feature, điểm mạnh/yếu, đề xuất cải thiện
- [ ] Lập danh mục chức năng
  Liệt kê toàn bộ feature từ README + src/ modules (wizard, terminal, broadcast, diff, kanban, pomodoro, dock, mascot)
- [ ] Phân tích Workspace & Wizard
  Đọc wizard.ts + workspaces.ts: luồng chọn folder, preset, grid 1-12, multi-select model
- [ ] Phân tích Terminal/ConPTY
  Đọc terminal.ts + ipc.ts + src-tauri: spawn agent, tree-kill, xterm.js, vòng đời pane
- [ ] Phân tích Tiling & Detach/Merge
  Trong main.ts: tiling frameless, control pill, drag reorder, detach ra cửa sổ riêng rồi merge
- [ ] Phân tích Diff review
  Đọc diff.ts + diffview.ts: cách review thay đổi từng agent, hiển thị diff
- [ ] Phân tích Tool dock (Kanban/Pomodoro)
  Đọc dock.ts, dockstore.ts, kanban.ts, pomodoro.ts: tính năng v0.2.0 mới thêm
- [ ] Phân tích Broadcast & Crew
  Đọc crew.ts: broadcast 1 lệnh tới nhiều agent, chọn nhóm agent nhận
- [ ] Đánh giá chất lượng & độ phủ test
  Soát các *.test.ts hiện có, chỉ ra module thiếu test và rủi ro
- [ ] Tổng hợp báo cáo phân tích
  Viết docs/feature-analysis.md: bảng feature, điểm mạnh/yếu, đề xuất cải thiện

## To do (1)

- [ ] Maestro Browser: agent lái trình duyệt ngay trong Maestro
  Spike xong (a6e8498): chép profile thật KHÔNG được trên Chrome 153 (mất login v20 + extension bị xoá); profile Maestro riêng + CDP thì được: nạp GravityCare từ thư mục code qua Extensions.loadUnpacked, đọc & bấm được app GravityCare (facebook.com/?window=auto-mkt), bấm icon extension được. Hướng mới: "soi gương" từng profile (cùng tên, extension unpacked nạp live, extension store thêm 1 chạm, đăng nhập 1 lần). Spec: docs/superpowers/specs/2026-09-24-maestro-browser-design.md
  - [ ] Rust: đọc profile thật từ Local State + extension của từng profile (unpacked: đường dẫn; store: id + tên)
  - [ ] Rust: tìm chrome.exe, mở/đóng Maestro Chrome (%APPDATA%/Maestro/chrome, port 0 đọc DevToolsActivePort, --enable-unsafe-extension-debugging), mỗi profile thật một profile gương cùng tên, ghi ~/.maestro/browser.json
  - [ ] Khi mở: Extensions.loadUnpacked mọi extension unpacked của profile đó; extension store thiếu thì hiện danh sách 'Thêm' (mở trang Web Store)
  - [ ] Cầu file browser-requests.jsonl: agent gọi tool lúc Chrome chưa mở thì Maestro tự mở đúng profile
  - [ ] maestro-mcp: browser_tabs/open/snapshot/click/type/press/select/hover/scroll/screenshot/wait (+ browser_eval tắt mặc định), tham số profile, playwright-core connectOverCDP
  - [ ] maestro-mcp: extensions_list, extension_open, extension_action (triggerAction theo tab target)
  - [ ] Mỗi agent sở hữu tab của mình (MAESTRO_AGENT)
  - [ ] UI: ô Browser xem trực tiếp (screencast), Dừng / Tôi làm tiếp, dấu 'đang duyệt web'
  - [ ] Settings → Browser: bật/tắt, profile nào dùng, Mở Maestro Chrome (đăng nhập/thêm extension), cho phép JS, hỏi trước khi gửi/đăng/thanh toán
  - [ ] Test: unit tool với Chrome headless + chạy thật với GravityCare; README

## Doing (3)

- [ ] Login: Auth core (Rust)
  Kho credential nguồn-chân-lý trong Rust: 1 username + 1 password hash (argon2id), lưu auth.json trong app-config dir.
  - [ ] Thêm crate argon2 (+ rand) vào Cargo.toml
  - [ ] Module auth.rs: load/save auth.json, hash argon2id + salt, không lưu plaintext
  - [ ] Thêm Auth vào AppState (state.rs)
  - [ ] Commands: auth_status / auth_setup / auth_verify / auth_lock / auth_change
  - [ ] Delay cố định khi verify sai (chống brute-force)
  - [ ] Đăng ký commands trong lib.rs invoke_handler
  - [ ] Unit test: setup, verify đúng/sai, change, reject setup khi đã có
- [ ] Fleet message không submit được (kẹt trong composer)
  Tin > 1 chunk (>120 ký tự) gõ vào pane Claude Code thì CR cuối bị TUI đọc là xuống dòng, tin nằm lì trong ô nhập; tin ngắn 1 chunk thì gửi bình thường, và CR của nó lại flush luôn cả cục đang kẹt → hai tin dính làm một. Bằng chứng: 2276/8331/8845 ký tự kẹt, 64 ký tự lọt (ByteWaker 15:22-15:33).
  - [ ] Đo ngưỡng thật: gõ 100/200/500/2000 ký tự vào một pane Claude Code, xem từ đâu CR hết ăn (nghi ngờ: hễ > 1 chunk 120 ký tự)
  - [ ] Sau khi bắn CR, đọc lại composer của pane; còn text thì bắn CR lại tối đa N lần rồi log cảnh báo (verify-and-retry — chắc ăn nhất, độc lập TUI)
  - [ ] Phương án gốc rễ: tin dài (>1-2k) ghi body ra .maestro/inbox/<id>.md, chỉ gõ một dòng trỏ tới file — đường qua PTY luôn 1 chunk
  - [ ] Gõ body bằng bracketed paste thật (ESC[200~ ... ESC[201~) để TUI không phải đoán, CR sau đó là Enter rõ ràng
  - [ ] Tăng SUBMIT_DELAY_MS vượt cửa sổ gom-paste đo được ở bước 1
  - [ ] Test typing.test.ts: bracketed paste, tin nhiều dòng, nhánh retry khi composer còn text
  - [ ] Chạy lại monitor send-watch.mjs trên fleet ByteWaker để xác nhận hết NOT_SENT/STUCK_COMPOSER
- [ ] Chat for every agent CLI: effort, permissions, background tasks, clearer Files changed
  Done so far (af3e677, 4ea3209, a1488a1, 8acd601): effort + permission chips, Background list, Files changed New/Edited/latest-reply/inline diff; chat for Claude, Codex (rollout JSONL) and opencode (SQLite). Left: the CLIs not installed here.
  - [ ] Gemini, Qwen, Copilot, Cursor, Goose, Aider: chat sources (not installed on this machine: install first or build from docs untested)
  - [ ] Try a live Codex and opencode agent in the chat (parsers checked on real sessions already)

## Done (63)

- [x] Fix copy khi bôi đen trong pane Claude Code (OSC 52)
  Claude Code tự xử lý selection + copy qua escape OSC 52; xterm.js của Maestro không có handler nên clipboard không được ghi. Fix: đăng ký OSC 52 handler trong terminal.ts.
  - [ ] terminal.ts: registerOscHandler(52) — parse 'c;<base64>', decode UTF-8, navigator.clipboard.writeText
  - [ ] Bỏ qua query '?' (không trả lời — tránh app đọc trộm clipboard)
  - [ ] Unit test trong terminal.test.ts cho parse/decode OSC 52
  - [ ] Verify thật: chạy app, bôi đen text trong Claude Code, paste ra ngoài
- [x] Redesign P1: Canvas + focus + tidy + identity
  Đổi .grid tiling thành canvas pane kéo-thả; focus zoom + rail avatar bé; tidy tile lấp màn; tên agent sửa được. (Spec 2026-07-21, mockup đã duyệt)
  - [ ] src/canvas.ts: layout state (pos/size per pane), nextSlot, tileToFit(count→cols/rows), persist per-workspace
  - [ ] main.ts: .grid → .canvas; pane header bar là drag handle (Pointer Events, no HTML5 DnD)
  - [ ] focusPane/exitFocus thay toggleMax: FLIP zoom + đẩy pane khác vào rail avatar bé (thay display:none)
  - [ ] Định danh: PERSONA_NAMES pool trong crew.ts, nameForNewPane, rename inline → cập nhật MAESTRO_AGENT + board assignee
  - [ ] styles/canvas.css: pane glass, header bar, rail, focus stage, scrollbar auto-ẩn
  - [ ] Unit test: nextSlot/tileToFit không đè, uniqueness tên, FLIP rect helper
  - [ ] Verify live: spawn 2/4/6 agent, tidy, focus/rail, rename
- [x] Redesign P2: Command bar + @mention + voice + delegation
  Thay broadcast bar bằng command bar 1 dòng; @tên autocomplete; voice→AI phân việc; vẽ delegation khi agent giao nhau.
  - [ ] #bcast → command bar 1 dòng (⚙ · input · mic · send · +Agent); spawn menu số lượng/CLI/custom
  - [ ] src/mention.ts: parseMentions(@tên → stdin đúng pane), autocomplete
  - [ ] Voice: nhận diện giọng nói → transcript → AI tách task/agent → dispatch (tích hợp giống WakerVoice)
  - [ ] Delegation event từ maestro-mcp (fleet_send) → vẽ connector + feed trong Fleet panel
  - [ ] Unit test: parseMentions, dedup delegation
  - [ ] Verify live: @tên gửi đúng, voice dispatch, agent giao nhau hiện connector
- [x] Explorer P1: Realtime watcher + file-ops backend (Rust)
  Tree tự cập nhật khi agent/CLI đổi file trên đĩa; bổ sung các lệnh fs còn thiếu (copy/move/trash/reveal).
  - [ ] Cargo: thêm crate notify (debounced) + trash
  - [ ] core/watch.rs: watch_start(root)/watch_stop, recursive, debounce ~150ms, bỏ qua .git/node_modules/target
  - [ ] Emit event 'fs-changed' kèm danh sách thư mục (rel) bị đổi, coalesce trùng
  - [ ] fs_copy (đệ quy) + fs_move (khác thư mục, tự né trùng tên) trong core/fs.rs
  - [ ] fs_trash: xoá vào Recycle Bin, nhận NHIỀU path, fallback xoá vĩnh viễn
  - [ ] fs_reveal: mở File Explorer và chọn đúng item (reveal_item_in_dir)
  - [ ] Đăng ký commands trong lib.rs + capabilities cho opener
  - [ ] Unit test Rust: copy đệ quy, move né trùng tên, trash nhiều mục, watcher lọc noise
- [x] Explorer P2: UX kiểu VS Code (multi-select, phím, menu, toolbar)
  filetree.ts lên chuẩn VS Code: chọn nhiều, điều khiển bàn phím, context menu đầy đủ, toolbar header, icon theo loại file.
  - [ ] Model phẳng cho hàng đang hiện (rows[]) + state expanded/selected tách khỏi DOM
  - [ ] Multi-select: click, Ctrl+click toggle, Shift+click chọn dải; anchor như VS Code
  - [ ] Bàn phím: ↑↓ di chuyển, ←→ đóng/mở, Enter mở, F2 rename, Delete xoá, Esc bỏ chọn, Ctrl+A
  - [ ] Xoá nhiều: 1 hộp xác nhận liệt kê N mục → fs_trash một lần
  - [ ] Context menu: New file/folder, Reveal in File Explorer, Open in terminal here, Cut/Copy/Paste, Duplicate, Copy path / Copy relative path, Rename, Delete
  - [ ] Toolbar trong header Explorer: new file, new folder, refresh, collapse all, hiện file ẩn (bỏ const showHidden=false)
  - [ ] Ô lọc nhanh (type-to-filter) trong panel
  - [ ] Icon theo đuôi file kiểu VS Code + màu; giữ ngôn ngữ thiết kế hiện có
  - [ ] Nhớ trạng thái mở/chọn theo workspace (localStorage) + auto-reveal file đang mở trong editor
  - [ ] Unit test: reducer select (ctrl/shift), flatten rows, expanded persist
- [x] Explorer P3: Kéo-thả di chuyển + đồng bộ editor
  Kéo (nhiều) mục thả vào thư mục để move — bằng Pointer Events; rename/delete/move phải cập nhật tab editor đang mở.
  - [ ] Drag nội bộ bằng Pointer Events (KHÔNG HTML5 DnD — WebView2 nuốt event)
  - [ ] Kéo được nhiều mục đang chọn; ghost đếm số mục; highlight thư mục đích, auto-expand khi hover ~600ms
  - [ ] Thả → fs_move; chặn thả vào chính nó/con của nó; Ctrl giữ = copy
  - [ ] Giữ nguyên drag ra terminal (path → PTY) như hiện tại
  - [ ] Editor: file đang mở bị rename/move → đổi đường dẫn; bị xoá → đóng tab/báo
  - [ ] Verify live trong app Tauri + screenshot review (dark/light)
- [x] Fix pane terminal quá ít dòng khi 3-4 agent
  Tile 2x2 chỉ ra ~12 dòng: tileToFit chừa đáy 84px chồng lên padding 56px của .main, padding .xterm dày, và font không co theo pane.
  - [ ] canvas.ts: bỏ double-reserve đáy (bottom 84 → 16) + test
  - [ ] workspace.css: .xterm padding 14/16 → 8/12, mask fade khớp 8px
  - [ ] terminal.ts: shrinkToFit + auto co font theo tile (target ~24 dòng, sàn 10px)
  - [ ] pane.ts: bật auto-fit cho mọi pane; focus/exit vẫn đổi base font
  - [ ] Test + tsc + build xanh
- [x] Canvas P1: Terminal trong suốt — chỉ còn chữ trên nền
  Bỏ nền đục #0b0d12 của xterm để ảnh nền workspace xuyên qua; auto đảo màu chữ theo độ sáng nền + cho chỉnh tay. Bẫy: WebGL không blend transparent → về #000, phải ép DOM renderer.
  - [ ] terminal.ts: allowTransparency + theme.background trong suốt (thay '#0b0d12' ở dòng 129)
  - [ ] KHÔNG attach WebglAddon cho pane trong suốt — tái dùng đường DOM renderer sẵn có (WEBGL_BUDGET / onContextLoss, terminal.ts:169-203); ghi comment lý do + đo lại perf 4 pane
  - [ ] Nền do AI CLI tự vẽ (ANSI SGR): chọn A) knockout map màu nền tối → transparent, hay B) chấp nhận chỉ ăn với CLI dùng default-bg. Ghi rõ chọn gì, mất highlight nào
  - [ ] Auto contrast: tính luminance nền (preset/màu/sample ảnh) → đảo TOÀN BỘ ANSI 16 giữa bảng chữ-tối và chữ-sáng, không chỉ foreground
  - [ ] Manual override trong Settings: light / dark / auto, lưu per-workspace cạnh background spec
  - [ ] Sàn dễ đọc trên ảnh rối: text-shadow hoặc scrim/backdrop-filter mỏng sau lớp chữ; đo contrast ratio (mục tiêu >=7:1, không dưới 4.5:1)
  - [ ] Cursor, selection highlight, ring pane focus phải còn thấy ở cả 2 theme
  - [ ] Slider opacity per-workspace trong Settings → Appearance (mặc định 0 = trong suốt hẳn)
  - [ ] Test: logic chọn theme theo luminance; tsc + suite hiện có xanh
  - [ ] Verify live: chụp claude VÀ opencode trên nền ảnh sáng + preset tối
- [x] Canvas P2: Pane dính liền nhau, gap = 0
  Các cửa sổ AI sát nhau thành một mặt liền, không chừa khoảng hở. Hiện tileToFit dùng gap:12, margin:18, top/bottom:16 và bo góc 12px (giả định có gap).
  - [ ] canvas.ts tileToFit: gap 12 → 0; cân lại margin/top/bottom (viền ngoài có thể giữ inset nhỏ, pane-với-pane phải liền)
  - [ ] Ranh giới giữa 2 pane chỉ là seam 1px, không hở nền; không để khe màu nền ở góc nơi 4 pane giáp nhau
  - [ ] canvas.css: sửa border-radius (dòng 29) — bo góc ngoài hoặc bỏ hẳn, góc trong vuông
  - [ ] Vẫn kéo/resize/nhận diện được từng pane; pane đang focus phải nổi rõ khi không còn gap
  - [ ] canvas.test.ts: cập nhật expectation cho tileToFit (sửa, không xoá test)
  - [ ] Verify live: 2 / 4 / 6 pane, kiểm tra không lộ khe và không chồng lấn
- [x] Canvas P3: Zoom giữ 4 pane còn đọc được
  Density zoom: Ctrl+scroll / Ctrl +,-,0 đổi cỡ chữ hiệu dụng và tile fit lại. Phải thắng auto-fit nudge (AUTO_FIT_DROP=4) chứ không đánh nhau với nó.
  - [ ] Zoom control: Ctrl/Cmd + scroll, Ctrl +/-/0, và một nút thấy được trên canvas
  - [ ] Lưu mức zoom per-workspace như layout
  - [ ] Density zoom: đổi font size hiệu dụng rồi re-fit cols/rows — KHÔNG CSS-scale (chữ sẽ mờ)
  - [ ] PTY resize bám theo mọi thay đổi: không cụt prompt, không loạn scrollback
  - [ ] Thứ tự ưu tiên rõ trong code + comment: zoom (ý người dùng) thắng auto-fit shrinkToFit/AUTO_FIT_DROP — nhắc commit bb959d2 vì sao có cap
  - [ ] Test: precedence zoom vs auto-fit; suite terminal/pane hiện có xanh
  - [ ] Acceptance: 4 pane tile trên cửa sổ 1080p, zoom mặc định đọc thoải mái
- [x] Canvas P4: Fleet sigil — dấu ấn sống giữa các pane
  Orrery — máy thiên văn: 3 vòng đồng tâm nét 1px, không glow/gradient. Vẽ bằng Canvas 2D ở z-index 1 (dưới pane, trên wallpaper) nên đọc xuyên qua terminal trong suốt, giữ nguyên tiling flush của P2. Mọi tham số sinh từ dữ liệu thật: góc theo rect pane, số cạnh lõi theo số agent, màu theo trạng thái.
  - [ ] Verify live: 2/4/6 agent, chụp màn hình, đo CPU khi fleet idle
  - [ ] Ngưỡng đọc được: chặn alpha để sigil không bao giờ tranh chữ terminal; kiểm trên pane đầy output
- [x] Pane Giám đốc luôn nổi bật nhất
  Nhìn phát biết ngay pane nào là thằng điều phối — bằng trọng lượng chrome tĩnh, không glow/gradient/animation.
  - [ ] pane.ts: gắn data-role="director" lên element pane + chip DIRECTOR trong title bar
  - [ ] canvas.css: header plate accent đặc + viền 1px accent (không đổi layout, gap 0 giữ nguyên)
  - [ ] panelayout.ts: avatar trong cloud-rail cũng mang dấu director khi ở focus mode
  - [ ] tidyLayout: director luôn về ô đầu tiên (top-left)
  - [ ] test + tsc + chụp màn hình 2 pane và 4 pane
- [x] Sếp: giao việc cho các terminal ĐANG MỞ trước khi đẻ agent mới
  Role conductor đã có (toggle #mConductor lúc spawn, CONDUCTOR_LAWS inject qua --append-system-prompt). Vấn đề: luật hiện tại bảo nó agent_spawn worker mới, không bảo dùng agent đang mở. Sửa luật + gom 1 nguồn.
  - [ ] Gom CONDUCTOR_LAWS: đang bị copy-paste 2 bản y hệt (main.ts:54 và pane.ts:82) — tách ra src/laws.ts, cả 2 import
  - [ ] Viết lại luật: BƯỚC 1 luôn là fleet_status; chia plan thành card rồi fleet_send cho từng agent ĐANG MỞ theo tên; CHỈ agent_spawn khi không còn agent rảnh
  - [ ] Luật giữ 1 dòng, không ký tự cmd.exe nuốt (& | < > % ! ^ ( ) " ') vì đi qua cmd /c
  - [ ] Nhân sự vẫn nhận lệnh trực tiếp từ tôi: fleet_send gõ vào PTY nhưng KHÔNG cướp focus/không chen giữa dòng tôi đang gõ — kiểm lại đường deliver trong bridges.ts
  - [ ] Đổi nhãn UI 'Conductor' → dễ hiểu (Director/Sếp) ở spawn modal + tooltip
  - [ ] Test: laws.ts không chứa ký tự cấm; verify live giao 1 goal cho Sếp với 3 agent đang mở → cả 3 nhận việc, không sinh agent thừa
- [x] Flow: dòng tin realtime Sếp → agents (dock tool thứ 5)
  Hiện fleet_send chỉ hiện arc 2.6s + toast rồi biến mất — không theo dõi được. Thêm tab Flow trong dock: danh sách tin nhắn from → to + nội dung + giờ, cuộn realtime. Event-driven hoàn toàn, không thêm IPC/polling/timer.
  - [ ] src/flow.ts: ring buffer cứng 200 tin (from, to, text, ts, wsId/paneId) — pure, unit-test được
  - [ ] Nối vào ĐÚNG 1 chỗ đã có: call site showDelegation trong bridges.ts (deliver) — không thêm watcher, không thêm lệnh Tauri
  - [ ] Dock tool thứ 5 'Flow': thêm nút .dr-btn trong index.html + ToolId trong dock.ts, mức tái dùng như fleet.ts
  - [ ] Rẻ khi đóng: panel đóng thì CHỈ push vào buffer, không đụng DOM; mở ra mới render; không rAF, không setInterval
  - [ ] Row: avatar+tên người gửi → người nhận, tin rút gọn 2 dòng (bấm để xem đủ), giờ mono; click row → revealPane đúng agent
  - [ ] Auto-scroll đáy trừ khi người dùng đã cuộn lên; badge số tin chưa đọc trên nút rail khi panel đóng
  - [ ] Test: ring buffer cắt đúng ở 200, dạng broadcast (to=null) hiển thị 'toàn đội', clamp text dài
- [x] Fleet switcher ⌘K — "which agent am I on"
  Command-palette overlay to jump to any of N agents + a persistent you-are-here marker. The real fix for 10 open agents. Ref: Cursor/Graphite/Vapi ⌘K palettes (Mobbin).
  - [ ] src/switcher.ts: overlay palette (search field + grouped list), pure helpers filterAgents/groupByStatus/orderRows (Needs you → Running → Idle)
  - [ ] Row = color avatar+initial · name · CLI badge · Director chip · status dot+LABEL (not colour-only) · cwd/branch · last-activity; current agent flagged 'On stage'
  - [ ] Keyboard: Ctrl/Cmd+K open, ↑↓ move, ↵ focus that pane (revealPane/focusPane), Esc close, Ctrl+1..9 quick-jump; role=dialog, listbox+aria-activedescendant, focus trap + restore
  - [ ] styles/switcher.css using existing tokens (Geist, --accent, glass); z-index above modal; reduced-motion crossfade
  - [ ] Wire open triggers: shortcut + a small fleet button; reuse workspaces/panes data, no new IPC
  - [ ] Unit test switcher.test.ts: filter, group order, quick-jump index, empty-fleet state
- [x] Focus-stage identity + stronger "Others" rail
  When a pane is opened large it must SHOUT whose it is; the right rail becomes a real, scannable agent switcher instead of a thin avatar column.
  - [ ] Focus header (panelayout.ts + canvas.css): big color avatar + name + CLI badge + Director chip + status LABEL + cwd/branch, so the on-stage agent is unmistakable
  - [ ] Rail rework: status-sorted (Needs-you on top), attention badge, active-ring on current, always-visible name under avatar, count header; stays legible at 10+ agents
  - [ ] Non-colour status: dot + label/shape; keep gap-0 tiling + director accent conventions intact
  - [ ] Rail row → focusPane; hook the same 'On stage' marker used by the ⌘K switcher
  - [ ] Verify live: focus with 2 / 6 / 10 agents, screenshot dark; contrast + reduced-motion pass
- [x] Command bar composer redesign (target chip · mic · send)
  Redesign #bcast.cmdbar into a premium composer: replace the cryptic '@name to target one' with a real target chip; add mic; refine send. Ref: Cursor context chip + Gemini/Copilot/Langdock composers (Mobbin).
  - [ ] index.html: target chip left of field (◎ Fleet ▾ / @avatar Name) opening the shared agent picker; mic button; keep +Agent; refine circular send
  - [ ] broadcast.ts + mention.ts: chip sets/read the active target (broadcast vs one pane), @mention still works, human placeholder that names the chip
  - [ ] voice.ts wired to mic (push-to-talk visual state) — no new backend
  - [ ] styles/broadcast.css: composer card, chip, mic, send, focus ring, sent/broadcast feedback; placeholder contrast ≥ 4.5:1
  - [ ] States: empty / typing / targeted / sending + reduced-motion; verify live send to fleet and to one agent
- [x] Redesign vision: Agent Inbox prototype
  Toàn quyền redesign: đổi Maestro từ window-manager terminal sang hộp thư agent (Queue → Stage → Review + ⌘K). Prototype tương tác + ghi chú thiết kế trước khi đụng code thật.
  - [ ] Chẩn đoán UI hiện tại (screenshot + code)
  - [ ] Chốt mô hình: task là đơn vị, trạng thái tự suy ra
  - [ ] Prototype HTML: Queue, Stage + decision bar, Review diff, ⌘K, New task, Board
  - [ ] Ghi chú thiết kế: cắt gì, giữ gì, lộ trình migrate
  - [ ] Publish artifact cho user xem
- [x] Prototype: Split, real terminal, Files/Settings, start screen
  Lấy lại những gì redesign D làm mất: xem nhiều terminal cùng lúc, gõ thẳng vào terminal, chỗ cho Files/Settings/History, màn chọn folder.
  - [ ] Split: 2–4 terminal cạnh nhau, ghim bằng Ctrl+click / P, phím W
  - [ ] Terminal gõ được: prompt nằm trong terminal, hộp quyết định nổi lên trên, thu nhỏ được
  - [ ] Dock: Files + Settings; Stage có tab History (replay, schedule, cost)
  - [ ] Màn bắt đầu: chọn folder, recent, preset
  - [ ] Test hành vi + chụp kiểm tra + commit
- [x] Agent Inbox redesign (build)
  Dựng thật bản redesign đã duyệt (design/redesign/maestro-inbox.html). UI mới sau cờ bật/tắt, backend Rust giữ nguyên. Mỗi dòng dưới đây là một giai đoạn.
  - [ ] ✓ P1 Đọc câu hỏi agent (số + menu mũi tên), trạng thái tự suy ra, History
  - [ ] ✓ P2 Cinema: headline, stage với tên việc, dock gọn, hộp hỏi 1 hàng
  - [ ] ✓ P3 Split tự xếp 4 agent mọi project, thẻ riêng có hộp trả lời
  - [ ] ✓ P4 Changes (Review), History, race tag + Compare, +/− dòng
  - [ ] ✓ P5 Ctrl K, New agent, màn bắt đầu, bật mặc định
  - [ ] Chờ bạn: dùng thử trên agent thật rồi quyết gỡ code UI cũ
- [x] Make hidden features discoverable
  Users don't know about shortcuts, right-click, Split, tools. Surface them without cluttering the screen.
  - [ ] 'What Maestro can do' sheet: ? / Ctrl+/ / palette / start screen link; tools open from it
  - [ ] ⋯ button on queue rows opens the agent menu
  - [ ] Just-in-time tips once each (right-click, Split, review) + fix stale first-run tip
  - [ ] Settings: Show tips toggle + Show tips again
  - [ ] Palette actions get one-line descriptions; Scheduled agents, Replays, Usage in palette
  - [ ] Shortcut hints in dock tooltips
  - [ ] Tests, screenshots, commit
- [x] Chat view for one agent (Queue), terminals in Split
  In Queue the agent on screen renders as a desktop-style chat (Cursor / opencode desktop) built from Claude Code's transcript; Split keeps terminals. Plus a friendlier Queue/Split switch.
  - [ ] Backend: read a Claude transcript incrementally; spawn claude with --session-id so each agent's file is known
  - [ ] Parser: transcript lines to messages, tool steps (Read/Edit/Bash/Search/Todo/MCP) with results (tested)
  - [ ] Chat view over the terminal (terminal keeps its size): messages, markdown, collapsible steps, diffs, command output, working indicator
  - [ ] Composer: send to the agent, Stop (Esc), hidden while a permission card is up
  - [ ] Chat | Terminal switch on the stage; Split always terminals; non-Claude CLIs stay terminal
  - [ ] Friendlier view switch (Mobbin): icons + clear labels instead of Queue/Split
  - [ ] Tests, screenshots, commit per step
- [x] Chat view: fill the space like a real agent app
  Mobbin (v0, AI Studio, Graphite, Cursor, Claude): side panel with plan, changed files and session; turn footers; composer tools; starters.
  - [ ] Model: per-turn timing, model, token usage, files changed, latest plan (tested)
  - [ ] Side panel (wide screens): Plan with progress, Files changed (+/-, opens Changes), Session (model, branch, folder, started, tokens)
  - [ ] Turn footer: Worked for Xs · time · Copy
  - [ ] Composer tools: model chip, / commands menu, context size
  - [ ] Empty conversation: starter jobs
  - [ ] Tests, screenshot, commit
- [x] Commands and model come from the agent's own CLI
  No hand-written lists: Claude's commands from its own init (claude -p /cost, free); model via the CLI's own /model; other CLIs open their own menus in the terminal.
  - [ ] Backend run_capture(program,args,cwd,timeout): run a CLI, return its output
  - [ ] cliprofile.ts: per-CLI profile (commands source, model command, model aliases)
  - [ ] Claude: discover commands/skills/plugins/model from init in the agent's folder, cached; searchable picker
  - [ ] Model picker in the chat composer: sends the CLI's /model
  - [ ] Terminal mode (any CLI): Commands types / so the CLI shows its own list; Model opens the CLI's own picker
  - [ ] Tests, commit
- [x] Agents resume their conversation
  Every start made a new Claude session, so restarting (app restart, Start again, Resume all) lost the chat. Resume the agent's own session; offer a new one on purpose; pick an earlier one.
  - [ ] Backend: does a session exist; list a folder's sessions with title and time
  - [ ] Boot: --resume <own session> when it exists, else a new --session-id
  - [ ] Stopped chat: Resume (continues) and New conversation
  - [ ] Earlier conversations: side panel list + right-click; picking one resumes it
  - [ ] Tests, commit
- [x] UX audit + polish toàn app (từng nút)
  Rà từng màn, từng nút, từng cài đặt của Agent Inbox; sửa hết lỗi tìm được.
  - [ ] Tour tự động chụp mọi trạng thái (Focus chat/terminal, Grid, queue, menu, palette, help, settings, modal)
  - [ ] Audit code tĩnh: a11y, focus, hover, disabled, title/aria, copy
  - [ ] Gom danh sách lỗi theo mức P0-P3
  - [ ] Sửa P0/P1
  - [ ] Sửa P2/P3
  - [ ] Chụp lại xác nhận + test + commit
- [x] Maestro for Chrome: extension cho agent thao tác Chrome thật
  ƯU TIÊN (học theo Claude in Chrome). ĐÃ XONG lõi: extension (id lhnncnhgomngapalcfncgehffmmibmbf), native host (bản copy maestro.exe), hub WebSocket trong Maestro, 16 tool browser_* trong maestro-mcp, mỗi agent 1 tab group riêng, Settings → Browser liệt kê profile + hướng dẫn cài. Đã chạy thật trên app GravityCare (đọc/bấm/gõ/điền/chụp). Còn lại bên dưới. Spec: docs/superpowers/specs/2026-09-24-maestro-browser-design.md
  - [ ] Đưa extension lên Chrome Web Store để khỏi bước Developer mode (cần tài khoản developer của bạn, $5)
  - [ ] Bản cài Maestro (maestro.exe) chưa có tính năng trình duyệt: cần build release mới
- [x] Chat view: paste images with Ctrl+V
  The Focus chat composer ignores images on the clipboard; pasting a screenshot should attach it like the terminal does.
  - [ ] Catch paste events with image items in the chat composer
  - [ ] Save the image to a temp file and attach it to the message
  - [ ] Show a thumbnail chip with remove before sending
  - [ ] Test with a Windows screenshot (Win+Shift+S)
- [x] Chat composer and conversation: every case a chat app handles
  The cases a chat with an agent should handle, beyond typing: files in (drop, @, attach, paste), history and drafts, and reading the conversation (jump to latest, code copy, file links, find).
  - [ ] @ mention: type @ for a fuzzy list of the agent's folder files (git ls-files, else a walk); arrows/Enter/Tab insert @path, Esc closes
  - [ ] Drop files on the chat: pictures become attachments with a thumbnail, other files and folders go in as @path (relative inside the folder)
  - [ ] Attach button (+): native picker, many files, same as a drop
  - [ ] Paste files copied in Explorer (not only pictures): saved and put in as @path
  - [ ] Up/Down in an empty composer walks back through what you sent
  - [ ] Draft kept per agent across switching agents and restarting Maestro
  - [ ] Your message shows at once as sending, until the conversation has it
  - [ ] Jump to latest when scrolled up and new messages arrive; never yank you down
  - [ ] Copy button on code blocks in answers
  - [ ] File paths in answers open in Maestro's code panel
  - [ ] Ctrl+F finds in the conversation
  - [ ] Tests + a look in the app, commit per piece
- [x] Chat: answer questions, context warning, auto-resume, edit & resend
  The four daily frictions in the chat view, shipped as one release.
  - [ ] Answer AskUserQuestion by clicking an option in the chat
  - [ ] Warn when context passes ~80% and offer /compact
  - [ ] Sending to a stopped agent resumes it first
  - [ ] Edit and resend a sent message
  - [ ] Tests + release
- [x] Repo & release cleanup
  Faster clean releases and a tidy repo.
  - [ ] Drop Mac jobs from release.yml
  - [ ] Ignore maestro.old*.exe and unstage it
  - [ ] Merge feat/agent-inbox into master
- [x] Per-agent cost
  Each agent shows its own tokens and cost from its session transcript.
  - [ ] Sum usage per session transcript
  - [ ] Show cost on the agent row / chat
  - [ ] Tests + release
- [x] Limit Guard: no agent left dead by a usage limit
  Detect agents stopped by a plan limit, resume them at reset, hand the task off to another CLI with a brief, warn near the limit.
  - [ ] Detect limit hits (Claude, Codex) and when they lift
  - [ ] Row shows 'Out of usage · back 19:09'
  - [ ] Auto-continue at reset (setting)
  - [ ] Hand off to another CLI with a brief from the transcript
  - [ ] OS notification at 90%; New agent shows plan headroom
  - [ ] Tests + release
- [x] Lighter memory: chat tail + terminal scrollback
  Open a chat with only the end of the conversation; measure and trim hidden terminals' scrollback.
  - [ ] Chat loads the last few MB, whole conversation on demand
  - [ ] Measure scrollback memory, trim for hidden agents
  - [ ] Tests + release
- [x] Chạy mượt: git, file, tin nhắn, agent, chat
  7 chỗ làm app giật/treo/mất tin tìm được trong đợt audit 2026-09-28.
  - [ ] A1 Git ngầm không tranh index.lock với agent: bỏ add -N, GIT_OPTIONAL_LOCKS=0, numstat, bỏ qua agent đã dừng (review.rs, tasks.ts)
  - [ ] A2 Lệnh fs_* (và capture_window, browser_profiles) thành async + run_blocking (core/fs.rs)
  - [ ] A3 outbox.jsonl / spawn-requests.jsonl: đọc theo byte offset, chỉ tới dòng hoàn chỉnh, chống tick chồng (fleetbridge.ts)
  - [ ] A4 Chat Codex/opencode nhớ file transcript đã tìm thấy, không quét lại mỗi 0,8s (chatsource.ts, codex.rs)
  - [ ] A5 Agent chết lúc khởi động không kẹt 'đang chạy'; agent đã thoát được dọn khỏi registry (bridges.ts, pane.ts, agent.rs)
  - [ ] A6 Khởi động agent không khóa registry trong lúc spawn ConPTY (commands.rs)
  - [ ] A7 Chat chỉ vẽ thêm phần mới thay vì dựng lại 160 tin mỗi bước (chatview.ts)
  - [ ] Test hồi quy cho từng mục + tsc + vitest + cargo test xanh
- [x] Merge một cú bấm
  Review: Merge = commit + merge + dọn worktree trong một bước; conflict thì gửi lại agent; có Discard branch. Theo prototype design/redesign/maestro-inbox.html.
  - [ ] Rust: lệnh merge gộp commit (nếu còn thay đổi) + merge + xóa worktree/branch, trả kết quả có cấu trúc (ok / conflict + danh sách file)
  - [ ] Conflict: abort sạch, nút 'Gửi lại agent' soạn sẵn tin nêu file conflict và bảo agent rebase/merge
  - [ ] Discard branch: xóa worktree + branch có xác nhận
  - [ ] UI Review: nút Merge/Send back/Discard như prototype, phím M, trạng thái Merged trong hàng đợi
  - [ ] Test Rust (repo git tạm) + test UI
- [x] Comment trên diff gửi lại agent
  Trong Review, bấm vào dòng diff để để lại comment; gom hết rồi gửi một tin cho agent kèm file:dòng và đoạn code.
  - [ ] Diff view: bấm số dòng/nút + để mở ô comment inline, sửa/xóa comment
  - [ ] Nháp comment giữ theo agent (không mất khi đổi màn)
  - [ ] Gửi: soạn một tin gom mọi comment (file:dòng, trích code, ý kiến) → gõ vào agent qua đường gửi tin sẵn có
  - [ ] Đếm comment chờ gửi trên nút Send back
  - [ ] Test
- [x] Chat: kiểm thử và sửa cho mượt như terminal
  Chat của Maestro còn nhiều chỗ kém terminal (VD chọn /command xoá mất nội dung đang gõ). Kiểm thử như tester chuyên nghiệp, lập danh sách lỗi, sửa từng cái kèm test.
  - [ ] Tái hiện lỗi /Commands xoá nội dung đang gõ
  - [ ] Kiểm thử trên app thật: gõ, gửi, dán, @file, /lệnh, phím tắt, câu hỏi, dừng, chạy lại, cuộn, tìm, sao chép
  - [ ] So với terminal: thao tác nào terminal làm được mà chat không làm được
  - [ ] Agent đọc code chatview.ts tìm ca lỗi khó thấy
  - [ ] Lập danh sách lỗi theo mức độ
  - [ ] Sửa từng lỗi kèm test hồi quy
  - [ ] Kiểm lại trên app thật, phát hành
- [x] Chat hiện chữ dần như terminal
  Câu trả lời hiện ngay khi Claude đang viết (đọc từ màn hình terminal), thay vì đợi từng khối ghi vào transcript.
  - [ ] Đọc phần trả lời đang viết dở từ màn hình (bỏ khung, thanh trạng thái, ô nhập)
  - [ ] Hiện thành bong bóng tạm, thay bằng bản thật khi transcript có
  - [ ] Không nhấp nháy, không nhảy cuộn khi đang đọc phía trên
  - [ ] Test với màn hình thật của Claude Code
  - [ ] Thử trên app thật
- [x] Báo khi agent làm xong
  Thông báo trên máy và trên điện thoại (dashboard) khi agent xong việc hoặc gặp lỗi, không chỉ khi cần bạn trả lời.
  - [ ] Phát hiện chuyển working → xong (Done/Ready to review) và working → lỗi/dừng
  - [ ] Thông báo hệ điều hành khi cửa sổ không được focus; bấm vào mở đúng agent
  - [ ] Dashboard điện thoại: hiện và báo việc xong
  - [ ] Cài đặt bật/tắt, không spam (gộp, bỏ việc quá ngắn)
  - [ ] Test
- [x] Mật khẩu cho dashboard
  Dashboard (điện thoại / mạng LAN) phải đăng nhập mới xem và gõ lệnh vào agent được. Thay các card Login: Auth core + Gate dashboard; khoá app desktop để sau.
  - [ ] Rust: lưu mật khẩu băm argon2id, lệnh đặt/đổi/kiểm
  - [ ] Dashboard: trang đăng nhập, cookie phiên HttpOnly, /api/* trả 401 khi chưa đăng nhập, chậm lại khi sai
  - [ ] Không cho bật chế độ LAN khi chưa đặt mật khẩu
  - [ ] Settings: đặt/đổi mật khẩu
  - [ ] Test Rust + thử bằng curl
- [x] Xem lại các session cũ
  Màn Sessions: danh sách phiên Claude của project (cả worktree của agent) và mọi folder, bấm vào là đọc lại cả cuộc trò chuyện, chỉ xem.
  - [ ] Rust: liệt kê phiên theo project gồm cả worktree, nhiều hơn 40
  - [ ] Đọc trọn một phiên theo id (phiên dài đọc dần)
  - [ ] Màn Sessions: danh sách + tìm theo tiêu đề + khung đọc dùng lại giao diện chat
  - [ ] Mở từ Ctrl K, menu agent và phím tắt
  - [ ] Test + thử thật trên app
  - [ ] Release
- [x] Chat nói chuyện thẳng với Claude (stream-json)
  Agent Claude ở chế độ chat chạy bằng giao thức có cấu trúc thay vì gõ phím vào terminal: gửi tin chắc chắn, chữ hiện dần, xin quyền bằng nút. Có công tắc trong Settings, cách cũ vẫn còn.
  - [ ] Thử giao thức stream-json: sự kiện, xin quyền, hỏi đáp, dừng, resume
  - [ ] Rust: chạy tiến trình không PTY, đọc/ghi từng dòng, kill cả cây
  - [ ] Pane: agent chế độ mới sống chung với agent PTY (restart, stop, trạng thái)
  - [ ] Chat: gửi tin, chữ hiện dần, dừng, xin quyền và câu hỏi bằng nút
  - [ ] Công tắc trong Settings + tab Terminal hiện nhật ký
  - [ ] Test + thử thật + release
- [x] Grid hides stopped agents
  Stopped/exited agents leave the Grid; they come back once resumed.
  - [ ] Filter Grid fill to live agents (inbox.ts layoutSplit)
  - [ ] Grid hint counts live agents only
  - [ ] Unit test for the filter
  - [ ] Typecheck + tests, commit
- [x] Remove chat from the stage
  Stage always shows the real terminal; drop chat view, Chat/Terminal switch, chatView + directChat settings and stream agents. Keep transcript parsing for History/Handoff/limits.
  - [ ] Map every chatview/streamagent dependency
  - [ ] Inbox stage: drop showChat/hideChat/termMode/view switch
  - [ ] Remove directChat stream agents (pane, bridges, tasks, new-agent)
  - [ ] Remove chatView/directChat prefs + Settings rows
  - [ ] Move shared helpers out of chatview, delete dead code + CSS
  - [ ] Typecheck, full tests, commit
- [x] Readable Grid
  The Grid shows only as many terminals as fit at a readable size, and grows the font when there is room.
  - [ ] Pure gridPlan(): count, arrangement and font from the stage size
  - [ ] layoutSplit uses it instead of splitMax alone
  - [ ] Note when some agents do not fit
  - [ ] Tests
  - [ ] Release
- [x] Resume button and latest session
  A Resume button on each stopped agent, and Resume carries on the conversation the agent last had, not an old one.
  - [ ] Rust: read ~/.claude/sessions/*.json (pid, sessionId, cwd, startedAt)
  - [ ] Follow each Claude pane's live session id (/clear etc.) and save it
  - [ ] Resume button on stopped queue rows
  - [ ] Resume button on the stage of a stopped agent
  - [ ] Tests
  - [ ] Release
- [x] Grid: one big terminal + agent cards
  The Grid shows the agent you work with as a big real terminal and every other live agent as a readable card beside it.
  - [ ] railPlan(): big terminal tile + rail, 1 or 2 card columns, font
  - [ ] glance from each agent's transcript: last reply, what it is doing
  - [ ] Cards: state, job, doing, reply, answer buttons; click makes it the big one
  - [ ] Screenshots at laptop / 1080p / 1440p
  - [ ] Tests + release
- [x] Shared agent context across CLIs
  Any agent with the maestro MCP reads what Claude/Codex/OpenCode did in the project (agent_context), hand over to an open agent, auto takeover on usage limit. Spec: docs/superpowers/specs/2026-10-22-shared-agent-context-design.md
  - [ ] convdigest.ts shared digest, handoff refactored onto it
  - [ ] mcp esbuild bundle importing ../src parsers
  - [ ] mcp context readers: Claude, Codex, OpenCode (node:sqlite), worktrees
  - [ ] agent_context tool + fleet.json cli/session names
  - [ ] MAESTRO_LAWS sentence
  - [ ] Hand over to an open agent + Hand over button on stopped agent
  - [ ] Auto takeover on usage limit (pref on by default)
  - [ ] Tests + real check both directions
  - [ ] Release v0.10.0
- [x] Remote web: running agents only, grouped by project
  Phone dashboard hides stopped agents (they show again once resumed), groups agents under their project, and skips screen snapshots of stopped panes.
  - [ ] Snapshot: skip screen text of stopped panes, send project info
  - [ ] Page: group rows by project, needs-you groups first
  - [ ] Page: hide stopped behind a 'Show N stopped' toggle
  - [ ] Terminal tabs and grid: running agents only, with project
  - [ ] Test, commit, release
- [x] Remove Maestro's own question card
  Agents' questions are answered in their terminal; drop the floating answer card, its chip, the Grid answer box under the terminal, the Settings toggle and Alt+1..9 / Alt+A.
  - [ ] Remove card, chip and Grid mini box
  - [ ] Remove askCard setting and its shortcuts
  - [ ] Update help, History wording, tests
  - [ ] Release
- [x] Grid shows which project each agent is in
  Grid cards grouped under project headings, the terminal names its project
  - [ ] Cards sorted by project, terminal's project first
  - [ ] Project heading above each group when the Grid spans more than one project
  - [ ] Big terminal header shows project · job
  - [ ] Tests + tsc
  - [ ] Release
- [x] Project memory shared by every agent
  Per-project memory in .maestro/memory.json: agents read/save it over MCP, you view/add/edit/delete it in Maestro and can send an entry to an agent to fix
  - [ ] Shared store module (parse, add, edit, delete, secret filter, caps) + tests
  - [ ] MCP tools memory_get/save/update/delete + memory in server instructions + law sentence
  - [ ] Maestro panel: list, add, edit, delete, open from project header/palette/agent menu
  - [ ] Ask an agent to fix an entry (pick a running agent of the project)
  - [ ] Tests, tsc, mcp build, release
- [x] Folding a project keeps its running agents
  Folding a project in the list hides only its stopped and merged agents.
  - [ ] Folded group shows only live agents
  - [ ] Up/down keys follow what is shown
  - [ ] Test
  - [ ] Release v0.10.9
- [x] Chrome: đưa Tidy + Zoom ra khỏi góc pane
  Tidy (.tidy-fab right:16/top:14) và pill zoom (#canvasZoom right:104/top:14) nổi ĐÈ lên góc phải-trên của .main — đúng chỗ nút header của pane trên cùng bên phải. Dời cả hai lên topbar .tb-right.
  - [ ] index.html: chuyển #btnTidy từ trong <main class="main"> lên .tb-right (trước .tb-div), đổi class sang .tb-btn
  - [ ] workspace.css: bỏ .tidy-fab absolute/glass; giữ .main{position:relative} vì delegation overlay + sigil vẫn cần
  - [ ] zoomui.ts: inject pill vào .tb-right thay vì canvas; bỏ right/top absolute, giữ logic .on ẩn/hiện theo canvas mode
  - [ ] Giữ nguyên phím tắt: Ctrl+Shift+G (tidy), Ctrl+wheel, Ctrl +/-/0
  - [ ] Verify live 4 pane: nút search/max/restart/kill của pane góc phải-trên bấm được, không còn vật gì nổi trên canvas
- [ ] Grid redesign: cards, filter chips, headers
  Soi Grid bằng snapshot preview (build-premium-website). Sửa: chip lọc 1 dòng + đếm số tabular, dòng doing không lặp chữ trạng thái (Working · 42m), bỏ chữ thừa đầu nhóm project, title card rõ tầng, timestamp hàng needs đủ contrast, tooltip chip ghi rõ phạm vi Grid. Review mắt mới độc lập: sweep sạch, 0 P0/P1 mở. Full suite 624 + tsc + build xanh.
- [ ] Grid filter cluster redesign (project menu + chips)
  Vẽ lại cụm lọc Grid (build-premium-website, theo chê "lọc xấu, chỗ chọn project xấu"): bỏ select native, thay bằng nút project (folder icon + tên + chevron) mở menu nổi có checkmark + đếm số agent từng project; chip status thêm dot màu trạng thái; gom project + chip vào một pill thống nhất. Menu: listbox semantics, Esc/outside-click đóng, Up/Down di chuyển, focus trả về nút. Full suite 626 + tsc + build xanh, snapshot 3 trạng thái đã soi.
- [ ] Stopped-pane Resume veil (nút Resume giữa terminal)
  Pane agent dừng hiện lớp phủ giữa terminal: tên agent, exit code, nút Resume ở giữa gọi pane.restart(). Driven by class .stopped. Test: pane.veil.test.ts (4) + pane.resume.test.ts (3). Full suite 630 xanh, tsc + build sạch. Đã phát hiện: Muse CLI chưa có trong CLI_PRESETS và không có session-args riêng nên resume Muse vẫn mở phiên mới, cần làm tiếp.
- [ ] Muse agents resume their conversation
  Agent Muse resume đúng phiên cũ (muse resume id) thay vì mở trắng. Preset muse trong CLI_PRESETS (--yolo), logo M + màu #f472b6. Backend mới muse_sessions/muse_session_exists đọc store sessions/YYYY/MM/DD/id. Boot mới capture id qua snapshot-diff có lock (log xuất hiện ~3s sau spawn). Test: 4 vitest + 2 Rust; full 634 vitest + 149 Rust xanh, tsc + build sạch. Còn lại: user tự bấm thử trong app dev (spawn Muse, chat, stop, Resume).
- [ ] Codex/opencode resume their session
  Codex (codex resume id, rollout filename map) và opencode (opencode --session id, sqlite) resume đúng phiên riêng bằng capture newest-since-spawn có lock per-CLI, chung khung với Muse. Bonus: cursor preset có --yolo. Cursor-agent (ls cần TTY, không dò được store), copilot (shim hỏng), gemini/aider/qwen/goose (chưa cài): chỉ document, chưa làm. Test: Rust + vitest args/picks; full 640 vitest + 151 Rust xanh.
- [ ] Read Muse conversations (cards/handoffs)
  Chat view đã bị gỡ (fad7120, stage luôn là terminal) nên đổi scope: đọc hội thoại Muse (started/inbox_steer/assistant_message/tool_calls+results/questions/model/todos + unwrap retained_frame) qua backend muse_transcript + musemodel.ts + SOURCES['muse'], phục vụ Grid cards glance, handoff, digest. Validate trên log thật 21.5k dòng: 13 user + ~74 replies + 300+ steps đúng. Full 648 vitest + 152 Rust xanh. History (sessionsview) vẫn chỉ Claude.
- [ ] Grid filter: narrow viewport + keyboard check
  Dock gọn dưới 1400px (seg icon-only, search nhỏ, chip padding 8px); hàng chip cuộn trong khi cửa sổ rất hẹp thay vì tràn. Verify bằng trình duyệt thật trên render inbox thật: 1280 đủ 4 chip, 1000 dock nguyên vẹn + menu không bị cắt, Tab order đúng khi menu đóng/mở (Enter/Esc/mũi tên đã có unit test). Live Tab bằng phím thật không chạy được do tab nền, dùng computed order thay thế.
- [ ] History reads Muse sessions
  History list trộn phiên Muse theo thời gian (tag Muse hồng, scope project vẫn chỉ Claude vì phiên Muse không gắn folder), đọc bằng muse_transcript + musemodel có sẵn. Backend muse_session_list: title = prompt đầu, count trong 4MB đầu. Test Rust + mixed matching; full 649 vitest + 153 Rust xanh.
