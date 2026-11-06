# ByteHive: Deterministic Ingestion và Pre-Execution Gate

Ngày: 2026-11-06. Trạng thái: chờ duyệt.

## 1. Mục tiêu

Chặn tin nhắn trùng lặp, không liên quan hoặc đã có kết quả trước khi chúng chạm tới PTY và context window của agent CLI. Mọi phép tính và tra bảng giá do mã thuần làm, LLM chỉ được phép bóc số và phân loại.

Tiêu chí thành công:

- Bốn kịch bản ở mục 13 chạy offline, không tốn token, luôn xanh.
- Tin bị chặn không tới PTY, người gửi thấy lý do và gửi lại được.
- Tin không chắc chắn thì đi qua agent như hiện nay. Gate không bao giờ chặn vì đoán.
- Đo được chi phí Haiku thật sau tuần đầu chạy, không dùng số ước lượng.
- Công tắc trong Settings, mặc định tắt. Bản phát hành không đổi hành vi cho tới khi bật.

## 2. Không làm

- Không thêm Redis hay cache tập trung. Cache là file trong `.bytehive/` của từng dự án.
- Không gate phím gõ tay, phím từ `/term`, WebSocket terminal, prompt lúc spawn, laws của Director, handoff, `CARRY_ON`, takeover khi hết limit, thả file vào pane.
- Không để LLM làm số học, quy đổi tiền hay tra bảng giá.
- Không viết lại nội dung tin coding. Chỉ chuẩn hóa khoảng trắng.
- Không cache kết quả của tác vụ coding.
- Không thêm dependency mới. Schema hai trường thì kiểm bằng hàm tay.

## 3. Quyết định đã chốt

| Câu hỏi | Quyết định |
|---|---|
| Điểm neo | Hàm mới `sendGated` trong `src/ingest/`, bọc `sendMessage` |
| Bộ phân loại | Luật thuần trước, `claude -p --model haiku --json-schema` khi luật không quyết được |
| Tác vụ mã thuần | Số liệu ByteHive, thêm tool theo một interface chung |
| Khi guardrail kết luận không cần xử lý | Chặn, lưu lại, có nút gửi lại |
| Kết quả coding | Không cache, chỉ chống lặp 10 phút cho tin giữa các agent |
| Agent đang chờ trả lời | Không chặn, tin đi thẳng |
| Nội dung coding | Không viết lại, giữ nguyên văn |

## 4. Điểm neo và luồng

`sendMessage` (`src/ipc.ts:82`) là nơi duy nhất mọi tin lập trình được gửi vào PTY. Raw keystroke và `/term` bỏ qua nó, và đó là chủ ý.

```
fleet_send ─┐
Kanban ─────┤
điện thoại ─┼─> sendGated ─> hash ─> cache ─> luật ─> Haiku ─> tool ─> sendMessage
Founder ────┘                     │        │        │        │
                                  │        │        │        └─ kết quả: trả cho người gửi
                                  │        │        └─ is_actionable=false: chặn, lưu
                                  │        └─ khớp luật: trả kết quả, không gọi Haiku
                                  └─ HIT: trả kết quả, 0 token
```

Đi qua gate:

- `fleet_send` (`src/bridges.ts:301-310`), nguồn `fleet`.
- Giao việc từ thẻ Kanban qua `sendToAgentById` (wiring ở `src/bridges.ts:159-192`), nguồn `kanban`. Thả kéo một thẻ vào pane là bản nháp, không gate.
- Nút "message" trên điện thoại (`src/dashboard.ts:220-232`), nguồn `phone`.
- Relay của Founder (`src/company/engine.ts:248-263`), nguồn `founder`. Làm sau cùng vì Ana đang sửa vùng này.

Không đi qua gate: mọi đường trong mục 2.

## 5. Thành phần

Thư mục mới `src/ingest/`, mỗi file một việc, phần lớn là hàm thuần.

| File | Trách nhiệm | Kiểu |
|---|---|---|
| `types.ts` | `Source`, `Request`, `Outcome`, `ToolResult`, `Tool` | types |
| `normalize.ts` | Chuẩn hóa khoảng trắng, số viết bằng chữ (`nghìn`, `triệu`, `k`), bí danh model | thuần |
| `hash.ts` | SHA-256 của nội dung đã chuẩn hóa | thuần |
| `rules.ts` | Bắt chào hỏi, cảm ơn, xác nhận; bắt yêu cầu chi phí và quy đổi tiền | thuần |
| `cache.ts` | Đọc, ghi, TTL, LRU, fallback bộ nhớ | I/O |
| `haiku.ts` | Gọi CLI, ép schema, timeout, kiểm kết quả | I/O |
| `tools.ts` | `estimate_cost`, `convert_currency` (thuần), `remaining_budget` (đọc company store) | hỗn hợp |
| `log.ts` | Vòng đệm 2000 dòng, ghi file debounce, đếm số | I/O |
| `gate.ts` | Ghép bốn bước, hàng đợi theo agent nhận | I/O |
| `index.ts` | Đọc Settings, nối vào các nơi gọi, API cho panel | wiring |

Hợp đồng chính:

```ts
type Source = "fleet" | "kanban" | "phone" | "founder" | "retry";

interface Request { source: Source; from?: string; to: string; text: string }

interface ToolResult {
  status: "ok" | "error";
  values: Record<string, number | string>;
  timestamp: number;
  metadata: { tool: string; version: string; cacheable: boolean };
}

interface Tool {
  name: string; version: string; cacheable: boolean;
  run(payload: Record<string, unknown>): Promise<ToolResult>;
}

type Outcome =
  | { kind: "answered"; via: "rule" | "tool"; result: ToolResult }
  | { kind: "sent"; via: "pass" }
  | { kind: "cache-hit"; result: ToolResult }
  | { kind: "blocked"; id: string; reason: string }
  | { kind: "failed"; error: string };

function sendGated(req: Request, deps: GateDeps): Promise<Outcome>;
```

`GateDeps` được tiêm để test chạy offline: `send`, `classify`, `isAwaiting`, `now`, `dir`, `settings`, `log`.

## 6. Bốn bước

### 6.1 Bước 1: hash và cache

- Khóa là SHA-256 của nội dung sau khi gộp khoảng trắng và cắt hai đầu.
- Mỗi entry lưu `at`, `toolVersion`, `result`. Khi đọc, nếu `toolVersion` khác bản hiện tại thì coi như MISS và xóa. `toolVersion` là hash của bảng giá, danh sách tool và phiên bản prompt phân loại, nên đổi giá là kết quả cũ tự hết hiệu lực mà không phải băm lại.
- HIT: trả kết quả, nhãn `CACHE_HIT (0 Token)`, không gọi luật, Haiku hay tool.
- TTL mặc định 7 ngày, chỉnh trong Settings. Tối đa 500 entry, đuổi entry cũ nhất.
- Chỉ ghi cache cho kết quả có `cacheable: true`.

### 6.2 Bước 2: phân loại

Thứ tự cố định:

1. Nếu `isAwaiting(agentId)` thì bỏ qua toàn bộ gate, đường `pass`. Một tin `ok` lúc agent đang hỏi là câu trả lời thật.
2. Chống lặp: với nguồn `fleet` hoặc `founder`, cùng cặp người gửi, agent nhận và hash mà đã gửi trong 10 phút thì chặn với lý do `loop`. Đây là chặn, không phải trả kết quả, nên khác cache.
3. Nếu văn bản rỗng sau chuẩn hóa thì chặn với lý do `empty`. Không ai cần gửi tin rỗng tới PTY.
4. Luật thuần: chào hỏi, cảm ơn, xác nhận (tối đa 4 từ, không có động từ, tiếng Việt và tiếng Anh) thì chặn với lý do `smalltalk`. Yêu cầu tính chi phí hoặc quy đổi tiền thì trả payload luôn.
5. Còn lại gọi Haiku. Prompt nói rõ: chỉ trả `is_actionable: false` khi chắc chắn tin không phải việc cần làm; không chắc thì trả `true`.
6. Kết quả Haiku đọc từ `structured_output`; thiếu trường đó thì thử parse `result` như JSON, vẫn không được thì đường `pass`. Kết quả phải qua kiểm tra trong code: đúng hai trường, `payload` là object hoặc null, `intent` (nếu có) thuộc danh sách tool. Sai thì đường `pass`.

Schema gửi CLI:

```json
{
  "type": "object",
  "properties": {
    "is_actionable": { "type": "boolean" },
    "payload": { "type": ["object", "null"] }
  },
  "required": ["is_actionable", "payload"],
  "additionalProperties": false
}
```

`payload` chứa `intent` cùng các trường đã bóc. `intent` lạ nghĩa là việc coding, cho đi qua nguyên văn.

Lệnh gọi:

```
claude -p --model haiku --output-format json --json-schema <schema>
  --max-turns 1 --tools "" --no-session-persistence
  --settings {"disableAllHooks":true}
```

Nội dung tin đưa qua stdin, không đưa qua argv: argv đọc được từ tiến trình khác cùng máy, và Windows giới hạn dòng lệnh khoảng 32k ký tự. `run_capture` hiện đặt stdin là null, nên thêm một lệnh Rust `run_capture_stdin` (mục 16).

Timeout 8 giây. Hết giờ, lỗi CLI, JSON hỏng hoặc sai schema đều dẫn tới đường `pass` và một dòng log `haiku.error`. Gate chỉ chặn khi có kết luận chắc chắn.

### 6.3 Bước 3: thực thi bằng mã thuần

Ba tool của bản đầu, tất cả chỉ đọc:

| Tool | Vào | Ra | Cache |
|---|---|---|---|
| `estimate_cost` | `model`, `inputTokens`, `outputTokens`, `cacheReadTokens?`, `cacheWriteTokens?` | `usd`, `breakdown` | có, theo `toolVersion` |
| `convert_currency` | `amount`, `from`, `to` | `amount`, `rate`, `asOf` | có, tỷ giá do người dùng khai trong Settings, đổi tỷ giá là đổi `toolVersion` |
| `remaining_budget` | không | `tokensLeft`, `tokenCap`, `jobId` | không, vì là số liệu sống |

`remaining_budget` đọc company store; chưa có company hoặc chưa có Job thì trả `status: "error"` với lý do rõ ràng, không bịa số.

`estimate_cost` gọi `priceFor` và `usageCost` trong `src/pricing.ts`. Không viết lại bảng giá. Model không có trong bảng thì trả `status: "error"`, không đoán.

Hai nhánh rẽ:

- Có `intent` khớp tool: chạy tool, trả kết quả, không gửi tin vào PTY.
- Không có intent hoặc intent lạ: gửi nguyên văn qua `sendMessage`, đường `pass`.

### 6.4 Bước 4: ghi cache và log

- Ghi cache cho kết quả `cacheable`, ghi đè entry cùng khóa.
- Mỗi tin một dòng log, chỉ chứa hash, không chứa nội dung.
- Tin bị chặn lưu vào danh sách chờ gửi lại, có nội dung, vì nút gửi lại cần nó.
- Kết quả trả cho người gửi, không trả vào PTY của agent nhận.

## 7. Dữ liệu lưu

Tất cả nằm trong `<dự án>/.bytehive/`, đã được `gitexclude.rs` loại khỏi git.

| File | Nội dung | Giới hạn |
|---|---|---|
| `ingest-cache.json` | `{version: 1, entries: {hash: {at, toolVersion, result}}}` | 500 entry, TTL 7 ngày |
| `ingest-blocked.json` | `{version: 1, entries: [{id, hash, source, from, to, text, reason, at}]}` | 100 entry, cũ nhất bị đuổi |
| `ingest.jsonl` | Mỗi dòng một lần chạy, chỉ hash và số đo | 2000 dòng gần nhất |

Ghi file theo cách của company store: ghi `.tmp` rồi `fs_rename`. Log ghi cả file sau 2 giây kể từ dòng cuối, không ghi từng dòng, vì backend chưa có lệnh append.

Settings, qua `prefs.ts` như các thiết lập khác:

| Khóa | Mặc định | Ý nghĩa |
|---|---|---|
| `ingest.enabled` | `false` | Công tắc chính |
| `ingest.ttlDays` | `7` | TTL cache |
| `ingest.haikuDailyCap` | `200` | Trần số lần gọi Haiku mỗi ngày |
| `ingest.loopWindowMin` | `10` | Cửa sổ chống lặp tin giữa agent |

## 8. Xử lý lỗi

| Lỗi | Hành vi |
|---|---|
| Đọc cache lỗi | Chuyển sang cache bộ nhớ, cảnh báo một lần, luồng vẫn chạy |
| Ghi cache lỗi | Vẫn trả kết quả, thử lại ở tin sau, sau hai lần lỗi liên tiếp thì chỉ dùng bộ nhớ trong 60 giây |
| File cache hỏng | Coi như rỗng, ghi lại từ đầu, không chặn ai |
| Haiku timeout, lỗi, sai schema | Đường `pass`, ghi log, không chặn |
| Vượt trần Haiku trong ngày | Đường `pass`, ghi log `haiku.capped` |
| Tool trả `status: "error"` | Trả lỗi cho người gửi, không gửi tin vào PTY, không cache |
| PTY đã chết | `sendMessage` trả false, outcome `failed`, tin không vào danh sách chặn |
| Ghi `ingest-blocked.json` lỗi | Vẫn chặn, cảnh báo, tin không gửi lại được cho tới khi ghi được |

Không có đường nào để một lỗi của gate làm mất tin: hoặc tin tới PTY, hoặc tin nằm trong danh sách chờ gửi lại, hoặc người gửi nhận `failed` ngay.

## 9. Riêng tư

- Log chỉ có hash, không có nội dung.
- Hash của tin ngắn có thể bị đoán bằng từ điển, nên file log cũng được coi là dữ liệu riêng tư: nằm trong `.bytehive/`, không commit, không gửi đi đâu.
- `ingest-blocked.json` chứa nội dung tin để gửi lại. Cùng quy tắc trên.
- Không gửi nội dung tin tới dịch vụ ngoài nào khác. Đường Haiku chạy qua Claude Code CLI có sẵn trên máy, dùng đăng nhập hiện tại, và nội dung tin sẽ tới Anthropic như mọi tin bạn gõ vào agent. Không thêm nhà cung cấp mới, không cần API key mới.

## 10. Trả kết quả cho ai

- Người gửi là agent: gửi một dòng kết quả ngắn vào pane của agent đó, đi thẳng `sendMessage`, không qua gate (nguồn nội bộ `ingest-result`). Tin này tốn ít context nhưng vẫn tốn, nên chỉ gửi khi có tool thực sự chạy.
- Người gửi là bạn (Kanban, điện thoại): toast có kết quả và nút copy; lịch sử 50 tin gần nhất nằm trong panel Ingest.
- Tin bị chặn: toast cho người gửi biết lý do. Không tự động gửi lại.

## 11. Giao diện

Panel Ingest nằm trong dock, cùng chỗ Kanban và Pomodoro:

- Hàng đầu: số tin hôm nay, số cache hit, số bị chặn, số lần gọi Haiku, USD của Haiku.
- Danh sách tin bị chặn: nguồn, người gửi, agent nhận, lý do, thời gian, nút `Send again` và `Delete`.
- Danh sách kết quả gần đây: intent, kết quả, đường đi (`rule`, `haiku`, `cache`).
- Nút mở Settings cho bốn khóa ở mục 7.

Chuỗi giao diện bằng tiếng Anh như phần còn lại của app. Bố cục chi tiết sẽ qua skill thiết kế trước khi code, không tự chế trong spec này.

## 12. Telemetry và chi phí

Mỗi dòng `ingest.jsonl`:

```json
{"ts":1790000000000,"hash":"ab12...","source":"fleet","from":"Ana","to":"agent-13","path":"haiku","ms":{"gate":2410,"classify":2300,"execute":1,"total":2411},"haiku":{"called":true,"in":412,"out":88,"cacheRead":0,"usd":0.0009,"estimated":false},"tokensSavedEstimate":0}
```

- Số token và USD của Haiku lấy từ kết quả JSON của CLI (`usage`, `total_cost_usd`). Nếu CLI thiếu trường đó thì tính lại bằng `priceFor` và đặt `haiku.estimated: true` cho riêng dòng ấy.
- Token tiết kiệm chỉ là ước lượng (ký tự chia 4), tên trường đã nói rõ điều đó. Không dùng số này để báo cáo như số thật.
- Trần Haiku đếm theo ngày từ log. Vượt trần thì tin đi qua, không chặn.
- Ước tính để lên kế hoạch: Haiku 4.5 giá 1 USD mỗi triệu token vào và 5 USD mỗi triệu token ra; một lần gọi 400 vào, 80 ra khoảng 0.0008 USD. Nếu 80% tin do luật và cache xử lý, 1000 tin mỗi ngày còn khoảng 200 lần gọi, tức khoảng 0.16 USD mỗi ngày. Con số thật sẽ thay thế sau tuần đầu.

## 13. Kiểm thử

Bộ phân loại, tool, cache và log đều nhận dependency tiêm, nên bốn kịch bản chạy offline bằng stub trong `src/ingest/gate.test.ts`. Chuẩn hóa và tool có test thuần riêng (`normalize.test.ts`, `tools.test.ts`); bốn kịch bản ghép chúng lại. Không gọi mạng, không tốn token, không phụ thuộc tốc độ máy.

| # | Kịch bản | Tin gửi | Kỳ vọng |
|---|---|---|---|
| 1 | Hợp lệ lần đầu | `báo giá 120k input, 30k output, model claude-sonnet-5` | Luật bóc `120000` và `30000`; tool trả `usd` xấp xỉ `0.54`; cache có đúng một entry; PTY không nhận tin |
| 2 | Câu tự nhiên khác | `250 nghìn token vào với 40 nghìn token ra của haiku thì hết bao nhiêu?` | Chuẩn hóa `nghìn` thành 1000, bí danh `haiku` thành `claude-haiku-4-5`; `usd` xấp xỉ `0.45`; khóa cache khác kịch bản 1 |
| 3 | Tin không liên quan | `cảm ơn nhé`, agent không chờ trả lời | `send` không được gọi lần nào; outcome `blocked` lý do `smalltalk`; entry nằm trong danh sách chờ gửi lại; 0 token |
| 4 | Gửi lại y hệt kịch bản 1 | cùng chuỗi | Outcome `cache-hit`; 0 lần gọi Haiku; 0 lần gọi tool; elapsed dưới 5ms; nhãn `CACHE_HIT (0 Token)` |

So sánh số thực bằng sai số `1e-9`, không so chuỗi đã format.

Test biên, ngoài bốn kịch bản:

- Khoảng trắng thừa ra cùng hash.
- TTL hết hạn thì MISS và entry bị xóa.
- File cache hỏng thì gate vẫn chạy, không ném lỗi.
- Haiku timeout, JSON hỏng, sai schema, `intent` lạ đều dẫn tới `pass`.
- Agent đang chờ trả lời thì tin `ok` đi qua.
- Hai agent gửi qua lại cùng nội dung trong 10 phút: tin thứ hai bị chặn lý do `loop`.
- Vượt trần Haiku thì `pass` và log `haiku.capped`.
- Ghi cache lỗi hai lần liên tiếp thì chuyển bộ nhớ trong 60 giây.

Kiểm tra độ ổn định, chạy tay một lần trước khi phát hành: 20 tin mẫu, gọi Haiku thật 5 vòng, mọi vòng phải ra cùng `is_actionable` và cùng `intent`. Vòng nào lệch thì ghi vào log và xem lại prompt. Đây là bù cho việc không đặt được `temperature`.

Kiểm tra trên app thật: bật công tắc, gửi một tin chi phí qua `fleet_send`, thấy kết quả trong pane người gửi; gửi lại y hệt, thấy `CACHE_HIT`; gửi một tin cảm ơn từ Kanban, thấy toast chặn và nút gửi lại hoạt động.

## 14. Giới hạn đã biết

- CLI không có cờ `temperature`, nên không đặt được `temperature: 0` như prompt yêu cầu. Bù bằng luật thuần, schema, kiểm tra trong code và bài kiểm tra độ ổn định. Spec này không hứa điều mà CLI không làm được.
- Đường Haiku thêm vài giây cho mỗi tin mơ hồ. Vì vậy luật chạy trước, timeout 8 giây, hết giờ thì cho qua.
- Luật cần bảo trì. Cách nói lạ sẽ rơi xuống Haiku, và nếu Haiku cũng không chắc thì tin đi qua.
- Cache theo từng dự án, không chia sẻ giữa các máy. Remote không dùng chung cache ở giai đoạn này.
- Chống lặp 10 phút chỉ áp dụng cho tin giữa các agent, không áp dụng cho bạn gửi.
- Nội dung tin bị chặn nằm trên đĩa dạng văn bản thường, trong thư mục đã loại khỏi git.

## 15. Thứ tự triển khai và rollback

| Giai đoạn | Việc | Công tắc |
|---|---|---|
| A | Module thuần, cache, log, test bốn kịch bản | Tắt, chưa nối |
| B | Nối `fleet_send`, Kanban, điện thoại qua `sendGated` | Tắt mặc định |
| C | Panel Ingest, Settings, đếm số | Tắt mặc định |
| D | Relay Founder, sau khi phần company của Ana xong | Tắt mặc định |

Bật theo bước: một tuần chỉ `fleet`, đo số lần chặn và số lần bấm gửi lại; nếu không có báo nhầm thì mở Kanban và điện thoại. Rollback là tắt công tắc, không cần gỡ code, vì mọi đường vẫn gọi `sendMessage` như cũ khi gate tắt.

Tiêu chí dừng nếu sai: có một tin thật bị chặn mà người dùng phải gửi lại bằng tay vì lý do sai, thì tắt công tắc và sửa luật trước khi bật lại.

## 16. Việc cần sửa ở Rust

Một lệnh mới `run_capture_stdin(program, args, cwd, timeout_ms, input)`, cùng cách chạy, timeout và kill tree như `run_capture`, nhưng ghi `input` vào stdin rồi đóng. Kèm một test trong `src-tauri/src/commands.rs` xác nhận chương trình nhận đúng stdin. Thêm wrapper `runCaptureInput` trong `src/ipc.ts`.

Không sửa gì khác ở backend.
