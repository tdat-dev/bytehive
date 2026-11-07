# ByteHive: Deterministic Ingestion và Pre-Execution Gate

Ngày: 2026-11-06. Trạng thái: chờ duyệt.

## 1. Mục tiêu

Chặn tin nhắn trùng lặp, không liên quan hoặc đã có kết quả trước khi chúng chạm tới PTY và context window của agent CLI. Mọi phép tính và tra bảng giá do mã thuần làm, LLM chỉ được phép bóc số và phân loại.

Tiêu chí thành công:

- Bốn kịch bản ở mục 13 chạy offline, không tốn token, luôn xanh.
- Tin bị chặn không tới PTY, người gửi thấy lý do và gửi lại được.
- Tin không chắc chắn thì đi qua agent như hiện nay. Gate không bao giờ chặn vì đoán.
- Đo được chi phí nhà cung cấp thật sau tuần đầu chạy, không dùng số ước lượng.
- Không đụng tới quota hay đăng nhập của Claude Code, Codex, OpenCode. Hết quota CLI thì gate vẫn chạy.
- Công tắc trong Settings, mặc định tắt. Bản phát hành không đổi hành vi cho tới khi bật.

## 2. Không làm

- Không thêm Redis hay cache tập trung. Cache là file trong `.bytehive/` của từng dự án.
- Không gate phím gõ tay, phím từ `/term`, WebSocket terminal, prompt lúc spawn, laws của Director, handoff, `CARRY_ON`, takeover khi hết limit, thả file vào pane.
- Không để LLM làm số học, quy đổi tiền hay tra bảng giá.
- Không viết lại nội dung tin coding. Chỉ chuẩn hóa khoảng trắng.
- Không cache kết quả của tác vụ coding.
- Không gọi CLI của agent coding (claude, codex, opencode, ...) để phân loại, và không dùng chung đăng nhập hay quota của chúng.
- Không thêm dependency mới. Schema hai trường thì kiểm bằng hàm tay.

## 3. Quyết định đã chốt

| Câu hỏi | Quyết định |
|---|---|
| Điểm neo | Hàm mới `sendGated` trong `src/ingest/`, bọc `sendMessage` |
| Bộ phân loại | Luật thuần trước, API ngoài (OpenAI-compatible) khi luật không quyết được |
| Nhà cung cấp mặc định | 9Router, model `cx/gpt-6-luna`, đổi được trong Settings |
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
điện thoại ─┼─> sendGated ─> hash ─> cache ─> luật ─> API ─> tool ─> sendMessage
Founder ────┘                     │        │        │        │
                                  │        │        │        └─ kết quả: trả cho người gửi
                                  │        │        └─ is_actionable=false: chặn, lưu
                                  │        └─ khớp luật: trả kết quả, không gọi API
                                  └─ HIT: trả kết quả, 0 token
```

Đi qua gate:

- `fleet_send` (`src/bridges.ts:301-310`), nguồn `fleet`.
- Giao việc từ thẻ Kanban qua `sendToAgentById` (wiring ở `src/bridges.ts:159-192`), nguồn `kanban`. Thả kéo một thẻ vào pane là bản nháp, không gate. Thẻ được bọc thành prompt có `Task:` và hướng dẫn `card_move`, nên nơi gọi truyền thêm `gist` (`cardGist` trong `src/dispatch.ts`): gate xét `gist`, còn tin giao đi và tin trong Held back vẫn là prompt đầy đủ.
  Kanban chờ kết quả của gate rồi mới gán thẻ: thẻ chỉ có người nhận và chuyển cột khi agent thực sự nhận tin. Thẻ bị giữ, được code trả lời hoặc giao thất bại thì đứng yên, và Conductor không gửi lại cho tới khi nội dung thẻ đổi. Send anyway làm nốt việc gán và chuyển cột (`card` trong `BlockedEntry`).
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
| `provider.ts` | Gọi API ngoài qua Rust, ép schema, timeout, kiểm kết quả | I/O |
| `tools.ts` | `estimate_cost`, `convert_currency` (thuần), `remaining_budget` (đọc company store) | hỗn hợp |
| `log.ts` | Vòng đệm 2000 dòng, ghi file debounce, đếm số | I/O |
| `gate.ts` | Ghép bốn bước, hàng đợi theo agent nhận | I/O |
| `index.ts` | Đọc Settings, nối vào các nơi gọi, API cho panel | wiring |

Hợp đồng chính:

```ts
type Source = "fleet" | "kanban" | "phone" | "founder" | "retry";

interface Request {
  source: Source; from?: string; to: string; toName?: string; text: string;
  gist?: string; // lời người viết khi text đã bị bọc; gate xét gist ?? text, giao text
}

interface ToolResult {
  status: "ok" | "error";
  values: Record<string, number | string>;
  timestamp: number;
  metadata: { tool: string; version: string; cacheable: boolean };
}

interface Tool {
  name: string; version: string; cacheable: boolean;
  run(input: Record<string, unknown>): Promise<ToolResult>;
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
- HIT: trả kết quả, nhãn `CACHE_HIT (0 Token)`, không gọi luật, API hay tool.
- TTL mặc định 7 ngày, chỉnh trong Settings. Tối đa 500 entry, đuổi entry cũ nhất.
- Chỉ ghi cache cho kết quả có `cacheable: true`.

### 6.2 Bước 2: phân loại

Thứ tự cố định:

1. Nếu `isAwaiting(agentId)` thì bỏ qua toàn bộ gate, đường `pass`. Một tin `ok` lúc agent đang hỏi là câu trả lời thật.
2. Chống lặp: với nguồn `fleet` hoặc `founder`, cùng cặp người gửi, agent nhận và hash mà đã gửi trong 10 phút thì chặn với lý do `loop`. Đây là chặn, không phải trả kết quả, nên khác cache.
3. Nếu văn bản rỗng sau chuẩn hóa thì chặn với lý do `empty`. Không ai cần gửi tin rỗng tới PTY.
4. Luật thuần: chào hỏi và cảm ơn (tối đa 4 từ, tiếng Việt và tiếng Anh) thì chặn với lý do `smalltalk`. Từ trả lời (`ok`, `yes`, `no`, `done`, `rồi`, `vâng`, ...) thì đi thẳng tới agent, không gọi API, vì chúng có thể là câu trả lời cho một câu hỏi viết thường (không bật trạng thái chờ) hoặc báo xong việc. Câu hỏi chỉ về chi phí thì trả `data` luôn; tin có dấu hiệu việc coding (động từ sửa, viết, thêm, tên file) hoặc dài quá 200 ký tự thì để bộ phân loại xem.
5. Còn lại gọi API ngoài. Prompt nói rõ: chỉ trả `is_actionable: false` khi chắc chắn tin không phải việc cần làm; không chắc thì trả `true`.
6. Kết quả là JSON trong `choices[0].message.content`; bỏ rào code fence nếu model thêm vào, rồi kiểm bằng hàm tay: đúng hai trường, `data` là object hoặc null, `intent` (nếu có) thuộc danh sách tool. Sai thì đường `pass`.

Schema gửi trong request:

```json
{
  "type": "object",
  "properties": {
    "is_actionable": { "type": "boolean" },
    "data": { "type": ["object", "null"] }
  },
  "required": ["is_actionable", "data"],
  "additionalProperties": false
}
```

System prompt đã chạy đúng với `cx/gpt-6-luna`, dùng nguyên văn khi code:

```
You classify one message for a coding-agent orchestrator. Reply with one JSON object and nothing else.
Shape: {"is_actionable": <boolean>, "data": <object or null>}.
Rules:
- is_actionable is false only when the message is clearly small talk: a greeting, thanks, acknowledgement or emoji, with no task and no question.
- is_actionable is true for anything else, including coding tasks and questions. When unsure, true.
- If the message asks to estimate the cost of tokens for a model, set data to {"intent":"estimate_cost","model":"<model id>","inputTokens":<number>,"outputTokens":<number>}.
- Token amounts written as 120k or "250 nghìn" mean 120000 and 250000.
- If the message asks to convert money, set data to {"intent":"convert_currency","amount":<number>,"from":"<code>","to":"<code>"}.
- Otherwise data is null.
Never explain. Never use markdown.
```

Request, giao thức OpenAI chat completions:

```json
POST <baseUrl>/chat/completions
{
  "model": "<model trong cấu hình>",
  "temperature": 0,
  "max_tokens": 300,
  "response_format": {
    "type": "json_schema",
    "json_schema": { "name": "ingest", "strict": true, "schema": "<schema ở trên>" }
  },
  "messages": [
    { "role": "system", "content": "<luật phân loại, tiếng Anh>" },
    { "role": "user", "content": "<nội dung tin>" }
  ]
}
```

`data` chứa `intent` cùng các trường đã bóc. `intent` lạ nghĩa là việc coding, cho đi qua nguyên văn.

Trường kết quả tên `data`, không phải `payload`: 9Router trên route `cx` thay chữ `payload` bằng một cụm khác trong cả câu trả lời lẫn văn bản thường, kiểm ngày 2026-11-06. Vì vậy prompt và tên trường tránh dùng chữ đó.

Header: `Authorization: Bearer <apiKey>` (bỏ khi key trống) và `Content-Type: application/json`. `baseUrl` phải bắt đầu bằng `http://` hoặc `https://`; sai thì Settings từ chối lưu.

Không gọi CLI nào cả, nên gate chạy được cả khi Claude Code, Codex, OpenCode đã hết quota hoặc chưa đăng nhập. Nhà cung cấp không hỗ trợ `response_format` thì đặt `jsonMode: "prompt"` trong cấu hình: bỏ khối đó và dặn model trả JSON thuần, code vẫn kiểm như trên.

Nội dung tin đi trong body request tới nhà cung cấp bạn chọn. Key nằm trong file cấu hình phía Rust và không bao giờ đi qua JS hay vào log.

Timeout 12 giây. Hết giờ, 401/403/404, 429, 5xx, JSON hỏng hoặc sai schema đều dẫn tới đường `pass` và một dòng log có lý do. Gặp 429 thì nghỉ 60 giây trước lần gọi kế, không dồn request. Gate chỉ chặn khi có kết luận chắc chắn.

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

Dữ liệu của gate nằm trong `<dự án>/.bytehive/`, đã được `gitexclude.rs` loại khỏi git.

| File | Nội dung | Giới hạn |
|---|---|---|
| `ingest-cache.json` | `{version: 1, entries: {hash: {at, toolVersion, result}}}` | 500 entry, TTL 7 ngày |
| `ingest-blocked.json` | `{version: 1, entries: [{id, hash, source, from, to, text, reason, at}]}` | 100 entry, cũ nhất bị đuổi |
| `ingest.jsonl` | Mỗi dòng một lần chạy, chỉ hash và số đo | 2000 dòng gần nhất |

Ghi file theo cách của company store: ghi `.tmp` rồi `fs_rename`. Log ghi cả file sau 2 giây kể từ dòng cuối, không ghi từng dòng, vì backend chưa có lệnh append.

### 7.1 Cấu hình nhà cung cấp

`~/.bytehive/ingest-provider.json`, một lần cho cả máy, không nằm trong dự án và không commit:

| Trường | Ý nghĩa |
|---|---|
| `baseUrl` | Gốc API. Form điền sẵn `https://9router.zoldify.com/v1`; đổi được sang `https://api.deepseek.com/v1`, `https://openrouter.ai/api/v1`, `http://localhost:11434/v1` |
| `model` | Model dùng để phân loại. Mặc định `cx/gpt-6-luna` trên 9Router |
| `apiKey` | Khóa API, lưu văn bản thường trong file của bạn. Để trống với Ollama hoặc LM Studio |
| `jsonMode` | `schema` (mặc định) hoặc `prompt` cho nhà cung cấp không hỗ trợ `response_format` |
| `timeoutMs` | Mặc định 12000 |

Key chỉ đọc và ghi phía Rust. Settings nhận key lúc lưu, còn lần đọc sau chỉ trả `hasKey`, không trả lại chuỗi khóa. Key 9Router không ghi trong spec và không commit; chỉ dán một lần vào Settings.

Settings, qua `prefs.ts` như các thiết lập khác:

| Khóa | Mặc định | Ý nghĩa |
|---|---|---|
| `ingest.enabled` | `false` | Công tắc chính |
| `ingest.ttlDays` | `7` | TTL cache |
| `ingest.dailyCallCap` | `500` | Trần số lần gọi API mỗi ngày |
| `ingest.dailyTokenCap` | `500000` | Trần token mỗi ngày |
| `ingest.loopWindowMin` | `10` | Cửa sổ chống lặp tin giữa agent |

## 8. Xử lý lỗi

| Lỗi | Hành vi |
|---|---|
| Đọc cache lỗi | Chuyển sang cache bộ nhớ, cảnh báo một lần, luồng vẫn chạy |
| Ghi cache lỗi | Vẫn trả kết quả, thử lại ở tin sau, sau hai lần lỗi liên tiếp thì chỉ dùng bộ nhớ trong 60 giây |
| File cache hỏng | Coi như rỗng, ghi lại từ đầu, không chặn ai |
| Chưa cấu hình nhà cung cấp, hoặc key sai (401/403) | Đường `pass`, log `provider.unconfigured` hoặc `provider.auth`, Settings hiện cảnh báo |
| API timeout, 404, 5xx, JSON hỏng hoặc sai schema | Đường `pass`, ghi log lý do, không chặn |
| 429 từ nhà cung cấp | Đường `pass`, nghỉ 60 giây trước lần gọi kế, log `provider.rate_limited` |
| Vượt trần gọi hoặc trần token trong ngày | Đường `pass`, ghi log `provider.capped` |
| Tool trả `status: "error"` | Trả lỗi cho người gửi, không gửi tin vào PTY, không cache |
| PTY đã chết | `sendMessage` trả false, outcome `failed`, tin không vào danh sách chặn |
| Ghi `ingest-blocked.json` lỗi | Vẫn chặn, cảnh báo, tin không gửi lại được cho tới khi ghi được |

Không có đường nào để một lỗi của gate làm mất tin: hoặc tin tới PTY, hoặc tin nằm trong danh sách chờ gửi lại, hoặc người gửi nhận `failed` ngay. Kết quả tính được mà không có ai nhận thì tin đi tiếp tới agent.

## 9. Riêng tư

- Log chỉ có hash, không có nội dung.
- Hash của tin ngắn có thể bị đoán bằng từ điển, nên file log cũng được coi là dữ liệu riêng tư: nằm trong `.bytehive/`, không commit, không gửi đi đâu.
- `ingest-blocked.json` chứa nội dung tin để gửi lại. Cùng quy tắc trên.
- Không gửi nội dung tin tới dịch vụ nào khác ngoài nhà cung cấp bạn cấu hình. Đó là API riêng, không dùng quota hay đăng nhập của CLI coding.
- Key API nằm trong `~/.bytehive/ingest-provider.json` dạng văn bản thường. Ai đọc được file trên máy thì đọc được key, nên đừng dùng key có quyền rộng hơn mức cần.
- Key không vào log, không trả về JS sau khi lưu, không xuất hiện trong thông báo lỗi.

## 10. Trả kết quả cho ai

- Người gửi là agent: gửi một dòng kết quả ngắn vào pane của agent đó, đi thẳng `sendMessage`, không qua gate (nguồn nội bộ `ingest-result`). Tin này tốn ít context nhưng vẫn tốn, nên chỉ gửi khi có tool thực sự chạy.
- Người gửi là bạn (Kanban, điện thoại): toast có kết quả; lịch sử 50 tin gần nhất nằm trong panel Ingest, giữ trong bộ nhớ cho tới khi đóng app. Panel mở bằng Ctrl+Shift+G, Ctrl K hoặc trang Help.
- Tin bị chặn: toast cho người gửi biết lý do. Không tự động gửi lại.

## 11. Giao diện

Panel Ingest nằm trong dock, cùng chỗ Kanban và Pomodoro:

- Hàng đầu: số tin hôm nay, số cache hit, số bị chặn, số lần gọi API, token đã dùng.
- Danh sách tin bị chặn: người gửi, agent nhận, lý do, thời gian, nút `Send anyway` và `Delete` (có Undo). Tin gửi lại đi thẳng tới agent, không qua gate, và chỉ rời danh sách khi tới được agent.
- Danh sách kết quả gần đây: intent, kết quả, đường đi (`rule`, `api`, `cache`).
- Nút mở Settings: các khóa ở mục 7 và form nhà cung cấp gồm Base URL, Model, API key, JSON mode, nút Test gọi thử một tin mẫu.

Chuỗi giao diện bằng tiếng Anh như phần còn lại của app. Bố cục chi tiết sẽ qua skill thiết kế trước khi code, không tự chế trong spec này.

## 12. Telemetry và chi phí

Mỗi dòng `ingest.jsonl`:

```json
{"ts":1790000000000,"hash":"ab12...","source":"fleet","from":"Ana","to":"agent-13","path":"api","ms":{"gate":2410,"classify":2300,"execute":1,"total":2411},"llm":{"called":true,"model":"<model>","in":412,"out":88,"usd":null},"tokensSavedEstimate":0}
```

- Token của API lấy từ `usage.prompt_tokens` và `usage.completion_tokens` trong kết quả, là số thật.
- USD chỉ ghi khi model có trong `src/pricing.ts`; nhà cung cấp khác thì để `null` và panel hiện token, không bịa giá. Bảng giá của `pricing.ts` hiện chỉ có model Claude, nên mặc định sẽ là `null`.
- Token tiết kiệm chỉ là ước lượng (ký tự chia 4), tên trường đã nói rõ điều đó. Không dùng số này để báo cáo như số thật.
- Trần gọi và trần token đếm theo ngày từ log. Vượt trần thì tin đi qua, không chặn.
- Đo thật với `cx/gpt-6-luna` qua 9Router ngày 2026-11-06: 5 tin mẫu, mỗi lần 347 tới 367 token vào và 31 tới 76 token ra, 1.8 tới 3.9 giây, 5/5 trả đúng cấu trúc. Với nhịp 500 lần gọi mỗi ngày, khoảng 220k token tổng, nằm dưới trần 500k.
- Chi phí thật của tuần đầu là số do nhà cung cấp báo, đối chiếu với `in`/`out` trong log. Không đặt trước một con số USD cho nhà cung cấp chưa chọn.

## 13. Kiểm thử

Bộ phân loại, tool, cache và log đều nhận dependency tiêm, nên bốn kịch bản chạy offline bằng stub trong `src/ingest/gate.test.ts`. Chuẩn hóa và tool có test thuần riêng (`normalize.test.ts`, `tools.test.ts`); bốn kịch bản ghép chúng lại. Không gọi mạng, không tốn token, không phụ thuộc tốc độ máy.

| # | Kịch bản | Tin gửi | Kỳ vọng |
|---|---|---|---|
| 1 | Hợp lệ lần đầu | `báo giá 120k input, 30k output, model claude-sonnet-5` | Luật bóc `120000` và `30000`; tool trả `usd` xấp xỉ `0.54`; cache có đúng một entry; PTY không nhận tin |
| 2 | Câu tự nhiên khác | `250 nghìn token vào với 40 nghìn token ra của opus thì hết bao nhiêu?` | Chuẩn hóa `nghìn` thành 1000, bí danh `opus` thành `claude-opus-5-5`; `usd` xấp xỉ `1.80`; khóa cache khác kịch bản 1 |
| 3 | Tin không liên quan | `cảm ơn nhé`, agent không chờ trả lời | `send` không được gọi lần nào; outcome `blocked` lý do `smalltalk`; entry nằm trong danh sách chờ gửi lại; 0 token |
| 4 | Gửi lại y hệt kịch bản 1 | cùng chuỗi | Outcome `cache-hit`; 0 lần gọi API; 0 lần gọi tool; elapsed dưới 5ms; nhãn `CACHE_HIT (0 Token)` |

So sánh số thực bằng sai số `1e-9`, không so chuỗi đã format.

Test biên, ngoài bốn kịch bản:

- Khoảng trắng thừa ra cùng hash.
- TTL hết hạn thì MISS và entry bị xóa.
- File cache hỏng thì gate vẫn chạy, không ném lỗi.
- Chưa cấu hình nhà cung cấp, key sai, timeout, JSON hỏng, sai schema, `intent` lạ đều dẫn tới `pass`.
- Model bọc JSON trong code fence vẫn đọc được.
- 429 thì tin đi qua, và lần gọi kế trong 60 giây bị bỏ qua.
- Agent đang chờ trả lời thì tin `ok` đi qua.
- Hai agent gửi qua lại cùng nội dung trong 10 phút: tin thứ hai bị chặn lý do `loop`.
- Vượt trần gọi hoặc trần token thì `pass` và log `provider.capped`.
- Ghi cache lỗi hai lần liên tiếp thì chuyển bộ nhớ trong 60 giây.

Kiểm tra độ ổn định, chạy tay một lần trước khi phát hành: 20 tin mẫu, gọi API thật 5 vòng, mọi vòng phải ra cùng `is_actionable` và cùng `intent`. Vòng nào lệch thì ghi vào log và xem lại prompt. Với `temperature: 0` thì lệch là lỗi của prompt hoặc model, không phải nhiệt độ. Chi phí cho lần chạy này chỉ vài cent.

Kiểm tra trên app thật: bật công tắc, gửi một tin chi phí qua `fleet_send`, thấy kết quả trong pane người gửi; gửi lại y hệt, thấy `CACHE_HIT`; gửi một tin cảm ơn từ Kanban, thấy toast chặn và nút gửi lại hoạt động.

## 14. Giới hạn đã biết

- Phụ thuộc mạng và nhà cung cấp ngoài. Máy offline, provider chết hoặc key hết hạn thì mọi tin đi qua agent; gate không chặn ai.
- Đường API thêm một chuyến mạng cho mỗi tin mơ hồ, thường dưới 2 giây nhưng có thể tới 12 giây. Vì vậy luật chạy trước, timeout 12 giây, hết giờ thì cho qua.
- `temperature: 0` đặt được qua API. Nhà cung cấp nào bỏ qua tham số này thì kết quả vẫn được kiểm bằng schema và bài kiểm tra ổn định.
- Luật cần bảo trì. Cách nói lạ sẽ rơi xuống API, và nếu API cũng không chắc thì tin đi qua.
- 9Router route `cx` viết lại chữ `payload` trong cả câu trả lời lẫn văn bản thường, nên trường kết quả tên `data`. Đổi nhà cung cấp vẫn giữ `data`.
- Cache theo từng dự án, không chia sẻ giữa các máy. Remote không dùng chung cache ở giai đoạn này.
- Chống lặp 10 phút chỉ áp dụng cho tin giữa các agent, không áp dụng cho bạn gửi.
- Nội dung tin bị chặn nằm trên đĩa dạng văn bản thường, trong thư mục đã loại khỏi git.

## 15. Thứ tự triển khai và rollback

| Giai đoạn | Việc | Công tắc |
|---|---|---|
| A | Module thuần, cache, log, test bốn kịch bản | Tắt, chưa nối |
| B | Nối `fleet_send`, Kanban, điện thoại qua `sendGated` | Tắt mặc định |
| C | Panel Ingest, Settings và form nhà cung cấp, đếm số | Tắt mặc định |
| D | Relay Founder, sau khi phần company của Ana xong | Tắt mặc định |

Trước giai đoạn B phải cấu hình nhà cung cấp và bấm Test trong Settings. Test gọi thật một tin mẫu, hiện kết quả và số token, để biết key và model đúng trước khi bật.

Bật theo bước: một tuần chỉ `fleet`, đo số lần chặn và số lần bấm gửi lại; nếu không có báo nhầm thì mở Kanban và điện thoại. Rollback là tắt công tắc, không cần gỡ code, vì mọi đường vẫn gọi `sendMessage` như cũ khi gate tắt.

Tiêu chí dừng nếu sai: có một tin thật bị chặn mà người dùng phải gửi lại bằng tay vì lý do sai, thì tắt công tắc và sửa luật trước khi bật lại.

## 16. Backend Rust

Ba lệnh mới trong module `src-tauri/src/ingest.rs`, đăng ký cạnh các lệnh khác:

| Lệnh | Việc |
|---|---|
| `ingest_provider_get` | Trả `{baseUrl, model, jsonMode, hasKey, timeoutMs}`, không bao giờ trả `apiKey` |
| `ingest_provider_save` | Ghi `~/.bytehive/ingest-provider.json` qua file tạm rồi rename. `apiKey` rỗng hoặc thiếu thì giữ key cũ |
| `ingest_classify` | Đọc cấu hình, POST `<baseUrl>/chat/completions`, trả `{content, usage: {in, out}, model}` |

- Dùng `reqwest` có sẵn, client tĩnh và khởi tạo rustls theo cách `planusage.rs:102` đang làm. Không thêm dependency.
- Thân request do một hàm thuần dựng, có test cho: URL ghép đúng khi `baseUrl` có hoặc không có dấu `/` cuối, body có `temperature: 0` và schema khi `jsonMode: "schema"`, không có `response_format` khi `"prompt"`, và đọc đúng `choices[0].message.content` cùng `usage`.
- Có test cho việc lưu cấu hình giữ key cũ khi form để trống key.
- Không có test gọi mạng trong CI.
- Lỗi trả về chỉ là mã và mô tả ngắn (`provider_unconfigured`, `http_401`, `timeout`, `bad_response`), không kèm key và không kèm nội dung tin.

Thêm ba wrapper `ingestProviderGet`, `ingestProviderSave`, `ingestClassify` trong `src/ipc.ts`. Không sửa gì khác ở backend.
