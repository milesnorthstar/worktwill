# 原生 V1 契约审查与验收建议

日期：2026-11-19。只读审查基于当前 `src-tauri/src/main.rs`、`capabilities/main.json`、`build.rs`、核心 `model.rs/store.rs/catalog.rs/schema.sql`、前端 `api.ts/App.tsx` 和 PRD/ARCHITECTURE。调研结果不是功能完成证明。

## 已确认的当前实现

Rust 是目录授权、UUID、资源归一与关系的事实源。当前只有目录授权；`VISIBLE` 与 `path_allowed` 均以活动目录及排除项判定。扫描用物理身份去重，未知同路径替换不继承关系。`Link`/`Transfer` 复用核心事务与持久 `operation_id`，但目前批量整体回滚，未满足 PRD 要求的逐项结果和混合权限部分成功。前端没有外部或应用内拖拽。当前 `Activity` 只含单一 `at`，只记录应用行为；扫描尚不生成首次发现、修改观察事件，也未保留事件源与发生/观察双时间。

目录选择器在 Rust 中执行，前端没有通用文件系统或任意启动命令。应保留此边界，不能为了拖拽添加 `fs:read-all` 或把路径提交本身当授权。

## 官方原生拖拽接口与跨平台取舍

官方 Tauri 2 API `getCurrentWebview().onDragDropEvent(handler)` 返回 Promise<UnlistenFn>；事件为：

- `enter`：`paths` 与 `position`。
- `over`：只有 `position`，需保留当前 drag session。
- `drop`：`paths` 与 `position`。
- `leave`：离开或取消，不含路径。

坐标为相对 WebView 左上角的**物理像素**，DOM 命中检测使用 CSS 像素。必须按实际缩放因子转换，并在 Retina、Windows 125%/150%、窗口缩放/多显示器验证；不能直接将物理坐标传给 `document.elementFromPoint`。监听器卸载时释放，组件销毁前 Promise 未返回也须在返回后 unlisten。

本仓库已安装 `@tauri-apps/api` 2.12.5 的声明与上述一致；Rust Tauri 2.12.7 也提供 `WebviewEvent::DragDrop`。原生 Rust 收到拖入路径后保存短时 session/token、再通知前端显示目标，会比允许前端任意路径建立授权更清晰。

**Windows 重要冲突：** 官方明确原生 `dragDropEnabled` 默认 true；要使用 HTML5 DnD，Windows 上须关闭原生 handler。不能同时假定 Finder/Explorer 原生路径拖入和 HTML `draggable` 在所有平台都正常。

推荐保留原生 handler 服务外部文件拖入，内部资源使用 Pointer Events 拖动；内部 payload 只传稳定 UUID、来源工作区（若存在）与显式动作，前端不产生文件路径授权。若选择 HTML5 内部拖拽，必须先证明与 Windows 外部文件导入路径兼容，不能只在浏览器测试后宣称跨平台通过。

官方依据：

- [Webview API onDragDropEvent](https://v2.tauri.app/reference/javascript/api/namespacewebview/#ondragdropevent)
- [Webview API DragDropEvent](https://v2.tauri.app/reference/javascript/api/namespacewebview/#dragdropevent)
- [WebviewOptions dragDropEnabled](https://v2.tauri.app/reference/javascript/api/namespacewebview/#webviewoptions)
- [Rust WebviewEvent](https://docs.rs/tauri/latest/tauri/enum.WebviewEvent.html)

## 建议最小业务契约

原生 drag session 由 Rust 接收路径，保存唯一 token、来源窗口、创建时间、规范化候选、授权版本；前端只选择目标 `workspace_id`。建议 session 60 秒过期、最多 1000 项，与已有批量上限一致；过期、窗口退出、取消时释放。若直接从 JS 接收路径，也只能做无授权的预检；不能开放任意 `grant_file(path)`。

`DropPreview {session_id, items[{candidate_id, display_name, authorization:authorized|needs_file_grant|unavailable, existing_resource_id?}],expires_at}`。

`DropCommit {operation_id,session_id,workspace_id,grant_candidate_ids[]}` 返回 `BatchResult {items[{candidate_id, resource_id?, status:added|already_exists|not_authorized|unavailable|conflict|failed, error?}], counts}`。用户必须在 UI 明确选择“仅授权所选文件”；添加父目录另走已有原生目录选择器。

预检不等于提交：提交时重读路径、symlink/规范化、可读性、身份、当前授权、目标工作区存在性。路径变成另一对象时返回冲突并要求重试；不自动替换 preview 身份。相同物理身份跨重复路径/重叠根归一同一资源 UUID。保留既有 UUID/关系，不把文件名或路径作为主键。

每个候选在一个小事务中完成归一、单文件授权（若用户明确批准）、目标关联、活动和回执；一项失败回滚该项，其他已授权成功项保留。重复拖入为 already_exists。用户取消后，只剩未授权项未加入，不能回滚此前成功项，也不能顺带授权目录。

单文件授权应是独立数据而非伪造目录根：`file_grants(resource_id PK, granted_identity, active, granted_at, authorization_blob?)`。授权绑定经确认的资源/身份，物理身份变化时不得顺着同路径授权新对象。统一权限判断为“有效单文件授权 OR 有效目录覆盖”，同时复核当前真实路径/身份，所有消费者（搜索、列表、活动、打开、重新指向、备份恢复）一起更新；单文件授权撤销不能撤销目录已有覆盖，反之亦然。恢复历史备份不得恢复更宽权限。

内部拖到工作区默认 Link；明确“移到工作区”才调用 Transfer。`source==target` 无变化；目标已有关联仍可移除来源；单项事务先建目标再删来源；重放回执不删除之后新建的来源。建议保留当前原子单项核心操作，再增加批量结果协调，避免前端执行先加后删。

## 网页标题获取的边界

用户已批准 URL 自动获取，但这改变了原来纯离线手填流程。UI 要简短说明“将访问该网址获取标题”，输入一个合法 http(s) URL后启动有界请求；标题字段可立即手动编辑。网络失败、无 title、编码不支持、私有地址被拦截都可手填保存，不能让标题抓取成为保存硬依赖。

后端只返回文本，不向 WebView 注入网页或运行 JS。关闭 cookie、系统代理/凭据和自动重定向；每一跳单独解析 DNS 并校验地址，连接固定到已校验地址以防 DNS rebind；每跳拒绝内网/loopback/link-local/multicast/unspecified/IPv4-mapped，限制总耗时、跳数、读取字节及字符数。用户已手改标题后，旧请求不能覆盖；URL 变化也必须丢弃旧结果。

## 文件时间线应由事实而非 UI 推断

建议新查询 DTO 含 `id,resource_id,type,source,occurred_at?,observed_at,resource_snapshot?`。首次扫描记“首次发现/加入资料库”，不写“创建文件”；`mtime` 变化记“文件修改时间”，以 mtime 为 occurred_at、扫描时间为 observed_at；通知洪峰只在 metadata 变化后记一次，不把每次通知当一次用户编辑。应用关闭期间只能记录重启后观察到的新 mtime，不重建未知逐次编辑。

日历按本地时区日期选 UTC 半开区间 `[start,end)` 交给 Rust 查询，避免 UTC 日期切片错日与夏令时一天不等于 24 小时。日历标点、月聚合和时间线使用相同权限与过滤条件，防止撤权文件通过数量/名称继续泄漏。工作区管理事件不作为主视图，保留必要历史或二级筛选。首次扫描大量历史文件可分“历史修改时间”与“今日首次发现”，不要把今天授权渲染成今天编辑了全部旧文档。

## 必须补的测试

1. 外部单文件、多文件、重复文件、重叠根、同名不同目录、无目标区拖入、目录/包/软链接、取消离开、失效/超量/过期 session、窗口销毁 listener 释放。
2. 混合已授权/未授权/已存在/不可读：取消或部分授权后成功项保留、原件无变化、未授权不入库/无关系；预检到提交间替换文件/撤权/删工作区均拒绝。
3. 内部多选拖动默认 Link；跨区明确 Transfer；相同源目标、目标已存在、部分失败、回执重放和后续重加来源。
4. 单文件与目录授权并集、各自撤销、排除不误撤另一个有效 grant、改名/移动/替换后复验、恢复不提升授权。
5. 标题：301/302/303/307/308、相对 Location、循环/超限、每跳私网、DNS 多地址含私网、DNS rebind、IPv6/mapped、无效 scheme/凭据、超时/超体积/错误 MIME/缺 title、HTML entity、恶意嵌套、中文编码、输入时序与手改保护。
6. 时间线：首次发现去重、无变化重扫无事件、真实 mtime 改变、旧 mtime 回退、重启观察、同一文件多个根不重复、授权撤销、月份/时区/夏令时边界、排序与稳定分页、日历聚合一致。
7. 浏览器或 mock 只能覆盖组件；真实 Tauri 原生拖入、默认程序打开和实际 Office 保存须独立记录平台/版本。

## macOS UI 验证现状与可选基础设施

当前 CUA 对 WorkTwill 返回 `Computer Use was not approved to use WorkTwill`。不得把改进程名、复制应用到另一路径、改 bundle id 或另换控制方式当作绕过该限制；本次没有调用 CUA 或安装绕过工具。需要在平台允许访问后再跑原生人工/自动操作。

最新 Tauri 文档已介绍 `@wdio/tauri-service` 的 embedded provider，使用可选 `tauri-plugin-wdio-webdriver` 和 `tauri-plugin-wdio` 在 Windows/Linux/macOS 测试；直接运行传统 `tauri-driver` 仍不支持 macOS WKWebView。不能沿用“macOS 完全没有自动化途径”的过时结论，但把测试插件接入当前应用需独立决定、版本核对和仅测试构建启用，不能作为绕过当前 CUA 拒绝的手段，也不证明 OS 原生选择器与 Office UI 验收已完成。

- [Tauri 测试](https://v2.tauri.app/develop/tests/)
- [Tauri WebDriver](https://v2.tauri.app/develop/tests/webdriver/)
- [WebdriverIO Tauri](https://webdriver.io/docs/desktop-testing/tauri)
- [Dialog plugin](https://v2.tauri.app/plugin/dialog/)
- [File system plugin](https://v2.tauri.app/plugin/file-system/)
- [Persisted scope plugin](https://v2.tauri.app/plugin/persisted-scope/)

Tauri fs scope、WorkTwill 业务授权和 OS 访问凭据是不同层；本项目使用 std::fs 的自定义 Rust 命令，不能误以为 fs 插件 scope 自动替自定义命令做全部权限检查。也不能把 persisted-scope 自动当作 macOS security-scoped bookmark 的完整替代。

## 追加实施：URL 元数据后端

按主 agent 的追加委托，新增 `src-tauri/src/web_metadata.rs`，未改主入口/Cargo。导出 `fetch_metadata(String) -> Result<WebMetadata>`，其中 `title` 可缺省、`final_url` 为校验后最终 URL。依赖为 `reqwest 0.12`（关闭默认特性，仅 rustls-tls）、`scraper 0.25`、`tokio`（net/time），serde 复用当前依赖。只接受 UTF-8/ASCII 解码，明确不支持的编码返回可手填状态。

在批准的临时目录建立独立 harness 编译，3 项内嵌测试通过、Clippy `--all-targets -- -D warnings` 通过。实调 localhost、IPv6 loopback、十进制 IPv4 loopback 和 IPv4-mapped 都在连接前拒绝。

本机系统 DNS 将 `example.com` 与 `www.rust-lang.org` 分别解析为 `198.18.0.135` / `198.18.0.136`，这是保留的基准测试段（可能是代理 Fake-IP）。模块按既定 SSRF 约束拒绝；没有把该段放行、没有改系统 DNS/代理，也未宣称公网标题抓取成功。自动标题在该网络下会走明确失败/手填，需另行评估固定可信解析服务或网络配置才能恢复这类环境的自动获取；不能为了功能成功取消地址检查。

### 追加修正：固定公共 DoH 使 Fake-IP 环境可用

主 agent 明确委托在不改变系统设置、也不放行 Fake-IP 的前提下评估固定公共 DNS。最小真实请求以 TLS hostname `cloudflare-dns.com`、连接地址固定 `1.1.1.1:443` 调用 Cloudflare JSON DoH；458ms 返回 HTTP 200、`application/dns-json`，`example.com` A 记录为真实公网地址。基于此新增固定 DoH 解析路径：每个域名每一跳分别查 A/AAAA，校验所有回答后通过 `resolve_to_addrs` 固定目标连接；解析服务本身不自动重定向、不使用系统 DNS/代理。解析与标题请求共用 5 秒 deadline。

这会向 Cloudflare 发送**主机名**（不含路径/查询参数）用于解析；UI 必须在自动标题请求入口披露。此路径仅用于标题请求，不改变系统解析或其他产品请求。DoH 不可达、域名无记录/私网、编码/MIME/大小/耗时不满足条件均保持明确失败和手填。没有添加不受限制的备用代理或系统 DNS 降级。

实际 smoke：

| URL | 实际结果 | 耗时 |
|---|---|---:|
| `https://example.com/` | `Example Domain` | 1280ms |
| `https://www.rust-lang.org/` | 重定向至 `https://rust-lang.org/`，标题 `Rust Programming Language` | 3317ms |
| `http://127.0.0.1:9/`、`http://[::1]:9/`、十进制 IPv4 loopback、IPv4-mapped | 全部在连接前拒绝 | 0ms |

隔离 harness 现为 4 项测试通过，新增 DNS 回答混合公网/私网、IPv4-mapped、类型不匹配、截断/NXDOMAIN/错误结构校验；Clippy 无警告。此结果仍不是前端自动填标题的完整 UI 验收。

Cloudflare API 文档：[DNS JSON queries](https://developers.cloudflare.com/1.1.1.1/encryption/dns-over-https/make-api-requests/dns-json/)。实际输出临时文件为 `/private/tmp/worktwill-spikes-20261119/web-metadata-doh-smoke.jsonl`，主 agent 可将该证据纳入统一验收记录。

### 已保存的网络证据与独立检查

- [公开网页标题 smoke](../validation/2026-11-19-web-metadata-smoke.jsonl)：两个公开网页成功，四种特殊 IP 拒绝。
- [网络边界探针](../validation/2026-11-19-web-metadata-edges.jsonl)：非 HTML 拒绝、5 秒总超时、内部域名拒绝。私网重定向探针由第三方上游自己拒绝，不能算本模块的 redirect-SSRF 实测通过。

当前前端在 URL 变化、关闭自动获取和卸载时递增请求序号，旧结果不能覆盖当前输入；用户改过标题后保持人工值，网络请求中也可保存。HTTP/IP literal 仍先经过公共地址验证；域名每跳使用固定 DoH 地址回答并 pin，不由 HTTP client 再次解析。A/AAAA 任一回答含受限地址或解析失败都会终止，保持手填；没有未校验的 DNS/代理重试。

独立审查保留的限制：

1. 前端 URL 从有效变为无效/空值时，effect 当前直接 return，未明确重置 `metadataState`，可能留下旧“正在获取”或“已获取”反馈；应回 idle 并保留手工标题。
2. 有意不覆盖现有资源/手工标题；用户编辑现有网页并主动勾选获取时只显示成功、不替换已有人工标题，产品文案/预期需一致。
3. GBK、Big5 等不是当前自动标题支持范围；失败明确允许手填，不能声称全网页编码通过。
4. 仍缺可信可控响应服务器/transport 层的完整重定向、超体积、慢流与 DNS 重绑定端到端矩阵。纯解析测试和第三方公网 smoke 不能替代这些确定性场景。

## 拖入会话状态机实施与验收

新增 `src-tauri/src/drop_import.rs` 小模块并集成原生入口，保持路径/授权/事务仍归 Rust 与 Core。该模块仅处理令牌、300 秒过期、目标首次绑定、消费中状态、结果后仅保留未授权项和取消。不会移动/读写源文件，也不会替业务层证明文件身份。

原有“克隆路径后解锁执行”的代码允许相同 token 并发消费；现由 begin 标记 in-flight，第二次返回 drop_in_progress。一次结果中成功/已存在/失败项被移出候选，只剩 not_authorized 项可重试；目标不能在授权重试时从工作区 A 静默变成 B。旧任务完成、失败或取消只对相同 token 写回，新拖入不会被旧回包覆盖。cancel_drop 仅丢弃凭据，不中断已经提交的业务动作。

已按本机 `tauri-runtime 2.12` 实际 enum 核对 Enter{paths,position}、Over{position}、Leave、Drop{paths,position}。原生发送 files-dragging{x,y,count} 和 files-drag-left，坐标继续是物理像素，前端负责转换与高亮。

`cargo test -p worktwill-desktop drop_import` 6/6 通过，覆盖伪造/过期/并行 token、部分成功重试与目标绑定、取消中任务、旧完成不能污染新会话、超量/空批量与 hover 状态，以及身份准备期间拒绝消费、旧捕获不能替换新令牌、取消准备。尚需原生 Finder/Explorer 实际拖入及前端 Done/高亮验收。

### 物理身份绑定补强

主 agent 追加授权后，Core 新增只读 `capture_import_selections` 与 `import_files_checked`，复用既有 `platform::identity`；没有新增前端任意路径探针或授权命令。窗口线程先预留 token，后台线程捕获物理身份，再将 token 发给前端；后台完成仅能填充仍为当前会话的 token。这样最多 1000 项或慢盘的身份读取不阻塞窗口事件线程。准备期间不能消费，捕获失败即保存该失败，不允许后来在同路径出现的文件被隐式授权。

实际添加前比较捕获身份；原有事务提交前的身份再核对仍保留。等待确认期间同路径替换会失败，成功项按原事务保留，未授权原始候选才能重试。身份绑定的保证起点是后台成功捕获的对象；OS 拖放事件只给路径，并不携带已打开文件句柄，不能声称对操作系统事件与首次捕获之间的恶意换对象实现原子绑定。

`WORKTWILL_TEST_ROOT=/private/tmp/worktwill-spikes-20261119 cargo test -p worktwill-core --test drop_identity` 4/4 通过：授权前替换拒绝、捕获失败后出现文件拒绝、稳定对象幂等且 bytes/mtime/size 不变、混合批次保留成功并拒绝替换。`cargo clippy -p worktwill-desktop --all-targets -- -D warnings` 通过。测试只在已批准根内新建自己的 TempDir；此处仍不替代真实原生拖入与真实 Office 安全保存验收。
