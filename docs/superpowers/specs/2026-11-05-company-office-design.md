# ByteHive Company: công ty AI và văn phòng 3D

Ngày: 2026-11-05. Trạng thái: chờ chủ dự án duyệt.

## 1. Mục tiêu

Biến ByteHive từ một danh sách agent phẳng thành một công ty có vai trò. Bạn chỉ nói với Founder, Founder lập kế hoạch, thuê người, giao việc. Bạn duyệt ở hai điểm: kế hoạch và merge. Văn phòng 3D là màn hình chính để nhìn và đi tới đúng chỗ.

Ví dụ mẫu: bạn gõ "thêm marketing cho gcare". Founder chọn vai trò Marketing, tạo thẻ duyệt. Bạn bấm Approve, một nhân vật mới ngồi vào bàn ở tầng gcare với đúng prompt, skill và tool của người làm marketing.

Tiêu chí thành công:

- Giao một yêu cầu nhiều bước cho Founder, việc chạy tới bước merge mà bạn chỉ phải bấm duyệt 2 lần.
- "Thêm <vai trò>" tạo được nhân sự làm việc thật.
- Mở app lên, trong 5 giây biết việc nào cần mình.
- Không agent nào tự nhân bản hay đốt token quá giới hạn mà bạn không biết.

Không làm ở bản đầu: nhiều công ty trên một máy, mạng nhiều máy, nhân vật đi lại tự do, cho agent "về nhà" để tiết kiệm RAM, tùy biến nội thất.

## 2. Các quyết định đã chốt

| Câu hỏi | Quyết định |
|---|---|
| Một tầng là gì | Một dự án (một thư mục). Tầng thượng là Founder, sảnh là việc chờ bạn |
| Founder được tự quyết đến đâu | Duyệt kế hoạch 1 lần, sau đó tự chạy trong giới hạn, bạn duyệt merge |
| Văn phòng nằm ở đâu | Màn hình chính. Bấm một người thì terminal trượt ra, tòa nhà lùi sang trái |
| Phong cách | 3D đồ chơi low-poly, nhân vật dựng bằng code |
| Cơ chế cũ | Được giữ, thay hoặc bỏ tùy tính hợp lý (mục 9) |

## 3. Kết quả nghiên cứu dùng làm luật

Nguồn đầy đủ nằm trong lịch sử phiên làm việc. Một số con số chỉ có từ nguồn phụ, các con số giới hạn dưới đây là lựa chọn của mình, chỉnh được trong Settings.

- Tối đa 3 cấp: Founder, Lead, nhân viên. Nhân viên không thuê được ai (Claude Code agent teams cấm lồng nhóm; Cognition ghi nhận một cấp quản lý là mô hình chạy tốt).
- Thuê người phải qua duyệt và có ngân sách (Paperclip dùng cách này).
- Đếm token từ transcript, không đếm tiền, và bỏ phần cache đọc lại vì nó tăng rất nhanh ở mọi phiên dài. Paperclip issue #339: agent trả theo gói thuê bao ghi $0 nên trần chi phí không kích hoạt, một agent đốt 19,8 triệu token trong một giờ.
- Mỗi vùng file một người viết. Người review có context sạch (Cognition).
- Lead hay tự làm thay hoặc kết thúc sớm (CrewAI, tài liệu Claude agent teams), nên Lead và Founder bị cấm sửa file bằng quyền tool, không chỉ bằng prompt.
- Agent con mặc định không báo cáo lại (Cognition), nên `card_done` là bắt buộc và app theo dõi.
- Chưa có nghiên cứu nào chứng minh văn phòng ảo giúp giám sát tốt hơn danh sách. Vì vậy thông tin cần hành động luôn ở dạng chữ trong thanh Needs you.

## 4. Mô hình tổ chức

### Cấp bậc và quyền

| Vai trò | Nhắn được cho | Được làm | Không được làm |
|---|---|---|---|
| Bạn | Mọi người | Mọi thứ, gõ thẳng vào terminal bất kỳ ai | |
| Founder | Lead, người làm một mình ở một tầng | Lập kế hoạch, đề xuất thuê, điều chuyển, đọc mọi tầng | Sửa file, tự thuê khi chưa duyệt |
| Lead | Founder, nhân viên tầng mình | Chia card trong tầng, review, gom báo cáo | Sửa file, thuê người, nhắn tầng khác |
| Nhân viên | Sếp trực tiếp | Làm card được giao, `card_done` | Thuê người, nhắn người ngoài sếp |

Lead chỉ xuất hiện khi tầng có từ 2 người. Tầng có 1 người thì người đó báo thẳng Founder. Khi tầng có người thứ 2, kế hoạch phải kèm một Lead.

Việc giữa hai tầng luôn đi qua Lead, Founder, Lead. Mỗi người thuộc đúng một tầng vì agent chỉ làm trong một thư mục. Founder điều chuyển bằng cách dừng ở tầng cũ và mở phiên mới ở tầng mới.

Luật cũ vẫn giữ: bạn luôn có quyền cao hơn Founder và Lead. Bạn gõ vào đâu thì agent đó làm việc của bạn trước.

### Thực thi quyền

Quyền được kiểm tra ở 2 chỗ. MCP tool trả lỗi ngay để agent biết, và app kiểm tra lại lúc giao tin. Danh tính lấy từ biến môi trường app đặt khi spawn, không lấy từ tham số do model điền. Thêm `BYTEHIVE_AGENT_ID` cạnh `BYTEHIVE_AGENT`.

Giới hạn cần nói rõ: đây là lan can, không phải sandbox. Agent chạy bypass có thể tự ghi thẳng vào file trong `.bytehive/` và mạo danh người gửi. Luật này chặn sai sót thông thường của agent, không chặn được một agent cố tình lách.

## 5. Dữ liệu: mỗi thứ chỉ ở một chỗ

```
~/.bytehive/company/
  company.json        tầng, nhân sự, giới hạn. Chỉ app ghi
  requests.jsonl      yêu cầu từ MCP, app đọc rồi xử lý
  jobs/<job-id>.json  sổ của từng Việc: yêu cầu gốc, kế hoạch, sự kiện, token
  roles/<role-id>/
    role.json         tên, phòng ban, cấp, CLI mặc định, tool, ngoại hình mặc định
    prompt.md         mô tả nghề
    skills/<name>/SKILL.md
  office/             thư mục làm việc của Founder
<dự án>/.bytehive/board.json   card của tầng, thêm trường job và assigneeId
```

```ts
interface Person {
  id: string;            // ổn định, không đổi khi đổi tên
  name: string;          // duy nhất trong công ty
  role: string;          // role id
  rank: "founder" | "lead" | "staff";
  floor: string | null;  // null với Founder
  boss: string | null;   // person id
  cli: string;
  look: Look;            // màu áo, tóc, đồ cầm tay
  session?: string;
}

interface Job {
  id: string;            // ví dụ J-0142
  title: string;
  ask: string;           // nguyên văn yêu cầu của bạn
  status: "planning" | "awaiting-plan" | "running" | "awaiting-merge"
        | "paused" | "done" | "rejected";
  plan: JobPlan | null;  // card và người thuê, mỗi mục ghi tầng của nó
  tokenCap: number;
  tokensUsed: number;     // token mới, không tính cache đọc lại
  tokenBase: Record<string, number>;
  people: string[];
}
```

`Pane.spec` thêm `personId`. Trường `role: "conductor"` được giữ tới khi xong mục 9 vì `src/limitguard.ts:176` đang đọc nó.

Chỉ app ghi `company.json`. MCP tool gửi yêu cầu vào `requests.jsonl`, giống cách `spawn` đang chạy, nên không có hai tiến trình cùng ghi một file. Cảnh 3D chỉ đọc, không có dữ liệu riêng, nên văn phòng và board không lệch nhau.

Mã Việc đi theo card, báo cáo và token. Tên nhánh theo Việc chỉ áp dụng cho người được thuê cho Việc đó ở Pha 1, người dùng lại giữ nhánh cũ. Bấm một Việc là thấy đủ nó đang ở đâu.

## 6. Luồng một Việc

1. Bạn gõ vào ô giao việc. App tạo Việc, đánh thức Founder.
2. Founder đọc danh sách nhân sự và board các tầng. Ưu tiên người đang rảnh, chỉ thuê khi không ai hợp.
3. Founder gọi `job_plan`: card theo tầng, người cần thuê, trần token. Thẻ duyệt hiện trong Needs you.
4. Bạn bấm Approve, Edit hoặc Reject. Sửa bằng lời thì Founder lập lại kế hoạch.
5. Approve xong app tự mở tầng nếu đang đóng, spawn người trong kế hoạch, tạo card, báo Lead.
6. Lead chia card, mỗi người một nhánh riêng và một vùng file. QA đọc kết quả ở phiên mới.
7. Nhân viên `card_done`. Lead review, gom báo cáo lên Founder.
8. Founder báo Việc xong. Needs you hiện thẻ merge, dùng luồng Review hiện có.

"Thêm marketing" đi đúng đường này với một kế hoạch chỉ gồm một người thuê. Toàn app chỉ có một kiểu duyệt.

Thuê thêm giữa chừng cũng tạo thẻ duyệt, không tự động.

### Khi có sự cố

| Tình huống | Xử lý |
|---|---|
| Người đang có card Doing mà rảnh quá 10 phút, chưa `card_done` | App nhắc sếp. Lần 2 hiện trong Needs you |
| Một card bị trả về 3 lần | Dừng vòng lặp, hiện trong Needs you |
| Việc dùng 80% trần token | Cảnh báo trong Needs you |
| Việc dùng 100% trần token | Việc chuyển `paused`, app ngừng chuyển tin mới của Việc đó và hỏi bạn. Lượt đang chạy của agent không bị cắt |
| Hết limit CLI | Giữ cơ chế bàn giao và tự chạy lại hiện có |
| Agent bị dừng khi đang có card | Mở lại bằng session cũ, không được thì hỏi bạn |
| Founder đầy context | Mở phiên mới, đọc lại `jobs/` đang chạy |
| Đóng dự án | Theo luật hiện tại agent bị dừng. Tầng tối đèn, ghế và session được giữ |

Giới hạn mặc định: 5 người mỗi Việc, 5 người dưới một Lead, 20 người toàn công ty, 5 triệu token mỗi Việc. Đây là số mình chọn, chưa có dữ liệu.

## 7. Vai trò và thuê người

Danh mục vai trò khởi đầu: Founder, Lead, Developer, QA, Designer, Marketing, Copywriter, Researcher. Bản mặc định đi cùng app, bản của bạn ở `~/.bytehive/company/roles` và được ưu tiên.

Nếu chưa có vai trò phù hợp, Founder viết bản nháp role (prompt, chọn skill từ danh sách cài sẵn, tool). Thẻ duyệt hiện đủ nội dung. Duyệt xong role được lưu để dùng lại.

Agent marketing claudekit trên máy có 32 cái đều gắn với GCare, nên chỉ dùng làm nguyên liệu và phải viết lại cho chung. Skill GCare chỉ gắn cho nhân sự ở tầng gcare.

Trình dựng lệnh `buildRoleLaunch(role, person, cli)` trả về `{ args, env, files }`:

| CLI | Prompt | Skill | Tool | Bản đầu |
|---|---|---|---|---|
| Claude Code | `--append-system-prompt-file` | `--plugin-dir` (plugin tạo tự động) | `--allowedTools`, `--disallowedTools` | Có |
| Codex | `-c developer_instructions` | Chưa gắn riêng từng agent được, tạm nhúng vào prompt | `-s` | Sau |
| OpenCode | `OPENCODE_CONFIG_CONTENT` với `--standalone` | Khóa `skills` | `permission` | Sau, chưa chạy thử trọn vẹn |

Bản đầu Founder và Lead luôn dùng Claude Code. Nhân viên cũng dùng Claude Code cho tới Pha 3.

Lưu ý khi làm:

- Prompt phải đi qua file. `laws.ts` đang bị giới hạn bởi cú pháp `cmd /c` và độ dài dòng lệnh. Đường `--append-system-prompt-file` mới có trong tài liệu, Pha 0 phải chạy thử.
- Claude khi resume dùng lại prompt đã ghi, nên đổi role thì phải mở phiên mới.
- Không dùng `--agent` vì nó thay prompt mặc định của Claude Code.
- `BYTEHIVE_LAWS` đang bảo agent chờ người dùng duyệt card. Người trong công ty cần bản luật riêng theo cấp, nếu không họ sẽ đứng chờ.

## 8. Giao diện

UI theo ngôn ngữ hiện tại của app (tiếng Anh). Văn phòng dùng logo honey cell làm biển trên nóc tòa nhà.

Ba mức, một trục:

1. **Building:** tòa nhà, thanh Needs you, ô giao việc cho Founder.
2. **Floor:** một dự án, các bàn, board trên tường, ô nhắn Lead.
3. **Desk:** terminal thật chiếm bên phải, tầng thu nhỏ bên trái, có tab Terminal, Changes, Report.

Đường dẫn trên cùng luôn hiện vị trí. Esc lùi một cấp khi terminal không focus. Khi terminal đang focus, mọi phím chuyển thẳng vào terminal theo luật hiện có, nên việc lùi dùng nút trên đường dẫn.

Thanh Needs you hiện ở cả ba mức, bấm một dòng là bay tới đúng chỗ. Inbox hiện tại vẫn giữ ở dạng danh sách để xem nhanh.

Trạng thái nhân vật chỉ lấy từ một hàm thuần `officeState(task)`, dựa trên `deriveTaskState`:

| Trạng thái | Hình ảnh |
|---|---|
| needs | Đứng dậy, bong bóng vàng, đèn bàn vàng |
| working | Gõ phím, màn hình sáng xanh |
| review | Đặt tập giấy lên bàn Lead |
| idle | Ngả ghế |
| stopped | Ghế trống, màn hình tắt |
| giao tin | Máy bay giấy chạy dọc thang máy tới đúng tầng |

Chỉ needs mới có bong bóng, để văn phòng đông người không rối.

Nhân vật dựng từ khối cơ bản: áo màu theo phòng ban, đồ cầm tay theo nghề, logo CLI thật trên thẻ đeo (tái dùng `clibrand.ts`). Founder tạo ngoại hình mặc định khi thuê, bạn chỉnh được ở Pha 3.

## 9. Cơ chế cũ

| Cơ chế | Quyết định | Lý do |
|---|---|---|
| Terminal, PTY, worktree, Review, memory, limit guard, usage, History | Giữ | Là phần làm việc thật |
| Board và MCP `card_*` | Giữ, thêm `job`, `assigneeId` | Card vẫn là đơn vị việc |
| `fleet_send`, `agent_spawn`, outbox | Giữ đường truyền, thêm kiểm tra quyền và gửi liên tầng | Đường này đã chạy tốt |
| `to` bỏ trống trong `fleet_send` | Đổi: nghĩa là gửi sếp hoặc cấp dưới, không phải cả tầng | Tránh gửi loạn |
| Director | Thay bằng Founder | Director chỉ là prompt, không có cây cấp bậc |
| Crew preset | Thay bằng danh mục vai trò | Vai trò có đủ prompt, skill, tool |
| Conductor semi, auto, pipeline | Tắt ở tầng thuộc công ty ngay từ Pha 1, xóa ở Pha 4 | Hai bộ cùng giao việc sẽ đụng nhau, chế độ auto còn tự duyệt card, phá điểm duyệt |
| `orchestrator/` (Python) | Xóa ở Pha 4 | App không dùng |
| `src/assets/sprites` | Xóa ở Pha 4 | Không code nào dùng, văn phòng dựng bằng 3D |

## 10. Rủi ro lớn nhất: WebGL trong WebView2

`src/terminal.ts:124` ghi rằng WebGL của xterm từng làm GPU đứng 15 đến 20 giây, treo cả app và mất context khi rảnh, nên `WEBGL_BUDGET = 0`. Cảnh three.js cũng dùng WebGL trên cùng WebView2. Đó là nhiều canvas trong suốt của xterm còn đây là một canvas, nên có thể không giống nhau, nhưng chưa có gì chứng minh là an toàn.

Vì vậy Pha 0 phải đo trước khi làm phần 3D. Tiêu chí đạt, với 12 terminal chạy cạnh cảnh three.js:

- Không khung hình nào đứng quá 200 ms khi mở modal, đổi tab, resize.
- Để rảnh 1 giờ, không mất context, hoặc mất thì tự phục hồi trong 2 giây.
- CPU dưới 5% khi cảnh đứng yên.

Nếu không đạt, thử theo thứ tự:

1. Chạy three.js trong Worker với OffscreenCanvas.
2. Tách cảnh sang một cửa sổ WebView riêng.
3. Bake sẵn tòa nhà và nhân vật từ Blender thành ảnh, zoom bằng CSS. Nhìn gần giống, không cần WebGL, nhưng mất camera tự do.

Pha 1 không phụ thuộc rủi ro này, nên công ty vẫn chạy được dù 3D phải đổi cách.

Thiết kế cảnh: three.js import động để không làm chậm lúc mở app, chỉ vẽ khi có thay đổi, tối đa 30 khung hình mỗi giây khi có chuyển động, dừng hẳn khi cửa sổ ẩn. Khi `prefers-reduced-motion`, camera cắt cảnh thay vì bay. Nếu WebGL không dùng được, app rơi về Inbox, Needs you vẫn hoạt động.

## 11. Các pha

**Pha 0: Chạy thử rủi ro.** Đo WebGL theo mục 10. Xác nhận `--append-system-prompt-file` và `--plugin-dir` chạy trên agent thật, có giữ khi resume. Code thử không đưa vào app.

**Pha 1: Công ty, chưa cần 3D.** Module tổ chức, `company.json`, Việc, role, `buildRoleLaunch`, luật theo cấp, quyền, gửi liên tầng, token cap, phát hiện kẹt. Màn hình tạm dạng DOM: ô giao việc, Needs you, cây nhân sự. Xong pha này ví dụ "thêm marketing" chạy thật.

**Pha 2: Văn phòng 3D.** Tòa nhà, tầng, bàn, camera, trạng thái, thay màn hình tạm của Pha 1.

**Pha 3: Nhân vật và nhiều CLI.** Chỉnh ngoại hình, role cho Codex và OpenCode.

**Pha 4: Dọn dẹp.** Bỏ những gì đã đánh dấu ở mục 9.

Mỗi pha phát hành riêng được.

## 12. Kiểm thử

- Logic thuần, viết test trước: kiểm tra quyền, xác thực kế hoạch, phát hiện kẹt, trần token, `buildRoleLaunch`, `officeState`.
- MCP: tool mới, từ chối đúng khi sai cấp, đọc ghi `requests.jsonl`.
- Đường dẫn đầu cuối với Claude Code thật: "thêm marketing", một Việc qua hai tầng, một card bị kẹt, chạm trần token.
- Giao diện: chạy trên WebView2 thật, không chỉ Chrome, vì rủi ro nằm ở đó.

## 13. Điểm bắt đầu trong code

- `src/panetypes.ts:8`: thêm `personId`.
- `src/pane.ts:337-347`: dùng `buildRoleLaunch`.
- `src/fleetbridge.ts:107`: `SpawnLine` đang làm mất `from`.
- `src/bridges.ts:299`: `deliver` chỉ tìm trong một thư mục, cần gửi liên tầng và kiểm tra quyền.
- `mcp/src/server.ts:374`, `mcp/src/fleet.ts:58`: tool mới. `board_get` nhận thêm `floor` để Founder đọc tầng khác. Card trong kế hoạch do app tạo khi bạn duyệt.
- `src/spawnmodal.ts:178`: `spawnForConductor`.
- `src/laws.ts`: luật theo cấp.
