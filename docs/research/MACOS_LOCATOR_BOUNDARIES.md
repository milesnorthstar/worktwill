# macOS 定位恢复：当前事实与安全实现边界

实施更新：本文保留019研究时快照。022已实现活跃独立file grant的同物理对象普通Bookmark定位；024已接入新授权目录物理锚点、普通Bookmark及暂停已有引用恢复，见[决定0020](../decisions/0020-single-file-native-locator.md)、[0022](../decisions/0022-root-native-locator.md)与[验收记录](../ACCEPTANCE.md)。025增加schema7历史授权来源及重新指向影响确认，见[0023](../decisions/0023-history-origin-and-relink-review.md)。原始段落中的“当前/schema3/尚未持久化”均属019时态。Document ID的[只读研究](MACOS_DOCUMENT_ID_READONLY_RESEARCH.md)未得到足够证据实施自动归并；安全替换与完整原生U8仍未闭环，本文建议不能整体算作完成。

日期：2026-11-20。范围：F09、U8 及下一轮 macOS locator 实现准备。

本文是源码与现有实验记录的只读研究结论，不是实现完成或验收通过记录。本轮仅新增本文，未修改运行时代码、依赖或其他文档，未运行 GUI、构建、测试或基准。产品行为仍以 [PRD](../../PRD.md) 为准，职责与验收边界以 [ARCHITECTURE](../../ARCHITECTURE.md) 为准。

## 1. 结论与实施顺序

下一轮可先独立实现**单文件及重要引用的同物理对象定位恢复**：Bookmark 只提供候选位置，Rust 重新核对原物理 identity、当前授权及候选对象，条件一致才更新路径。这个增量不需要让 Bookmark 成为身份或权限事实源。

**整个授权 root 的定位恢复属于跨层授权扩展。** 当前 root 没有历史物理身份锚点；安全支持 root 外部改名、移动，须同时处理迁移、有效授权范围、扫描代次、排除、暂停、查询过滤和监听路径，不能只给 `scan_roots.path` 增加一次 Bookmark 解析赋值。

以上两类增量均不解决物理 identity 改变后的 Office 安全保存。当前不能用同路径、同名、mtime、大小、Bookmark 解析成功或 `stale` 状态证明文档延续。

## 2. 当前生产实现与 F09/U8 事实

| 项目 | 当前事实 | 证据边界 |
|---|---|---|
| macOS 文件 identity | `dev + ino + birthtime + birthtime_nsec` | 是当前保守物理对象键，不是已验证的文档延续身份 |
| 受管目录内改名、同卷移动 | 扫描按 identity 找到原资源，更新路径，复用 UUID | 需要扫描能在有效授权范围内发现目标；不是原生 locator 主动找回 |
| 独立授权文件移位 | 当前复验和打开依赖已存路径 | 没有 Bookmark 自动定位，改名、移动可能需要重新指向 |
| 整个授权 root 移位 | root 当前保存路径，没有 identity/Bookmark | 根本身被改名、移动后，不会通过原生定位恢复 |
| 同路径物理对象替换 | 原引用拒绝打开新 identity，旧 UUID 和关系保留 | 扫描可能发现独立的新资源；人工重新指向/合并须明确确认 |
| Bookmark、Document ID | 只在 Swift spike 中探测 | 生产 Rust/schema 尚未持久化或启用完整恢复机制 |
| Windows | 有 File ID 读取实现 | 没有本轮 NTFS、OpenFileById、Object ID 或真实 Office 验收；本文不扩展 Windows 支持 |

当前源码指针：

- [`platform.rs`](../../crates/worktwill-core/src/platform.rs)：`identity`、`safe_open`。macOS 分支读取物理键，没有生产 Bookmark/Document ID 调用。
- [`store.rs`](../../crates/worktwill-core/src/store.rs)：`resolve_resource` 规范化缓存路径，核对当前授权及 identity；自身不搜索新位置。
- [`catalog.rs`](../../crates/worktwill-core/src/catalog.rs)：`scan_all`、`scan_root`、`commit_batch` 按 identity 归一资源，并在提交前重新核对文件。
- [`ingest.rs`](../../crates/worktwill-core/src/ingest.rs)：`refresh_file_grants`、`import_one` 复验单文件及建立当前归属。
- [`schema.sql`](../../crates/worktwill-core/src/schema.sql)、[`migrations.rs`](../../crates/worktwill-core/src/migrations.rs)：当前 schema v3；`resources.identity` 已存在，`scan_roots` 没有物理 identity 或原生 locator 字段。
- [`workflows.rs`](../../crates/worktwill-core/tests/workflows.rs)：改名/核心重开、同路径替换不继承等核心回归。
- [`relink_grants.rs`](../../crates/worktwill-core/tests/relink_grants.rs)：原身份授权沿用、新身份授权隔离、人工合并及确认期间替换的回归。

### 2.1 WPS 证据必须按时间和层级读取

[`spikes/RESULTS.md`](../../spikes/RESULTS.md) 保留早期尝试：当时没有成功进入 WPS 空白文档，也没有执行保存。这是历史事实，不能用来否认后续已经发生的保存观察。

后续 [`ACCEPTANCE.md` 的“真实 WPS 保存”](../ACCEPTANCE.md#真实-wps-保存) 记录了 macOS 15.3.1、APFS、WPS 12.1.26046 的合成 DOCX 操作：两次原位保存，其中一次在核心观察进程退出期间；关闭重开；另存为同目录副本。原 inode、原 UUID、两个工作区和收藏保持，副本使用独立 UUID 且不继承关系。原始核心输出为：

- [第一次保存](../validation/2026-11-19-wps-save-one.json)。
- [第二次保存及副本](../validation/2026-11-19-wps-save-copy.json)。

这些记录支持**这组具体 WPS 原位保存/另存为操作的核心引用连续性**。核心观察进程退出不等于正式 WorkTwill 窗口退出；尚不能提升为完整 U8、Word、安全替换、多卷、云目录、系统重启或签名包支持。

`SAVE_ONE_20261119`、`SAVE_TWO_OBSERVER_CLOSED` 是 DOCX 内容标记，用来观察编辑内容是否保存。Swift 的 `.worktwill-fixture-root` 是实验样本目录护栏。两者均不是生产身份机制；本方案不向源文件、xattr 或源目录写入追踪 marker。

### 2.2 原生实验说明了什么

[`spikes/macos_identity.swift`](../../spikes/macos_identity.swift) 及 [S02 结果](../../spikes/RESULTS.md#s02已有结果) 已证明本机普通 Bookmark 创建、解析和新进程解析 API 可以调用。实际样本中：

- 改名、同卷移动后，Bookmark 可以返回新位置，物理对象保持。
- 原子写入、FileManager 替换后，Bookmark 可以返回保存后的路径，但物理对象改变。
- 删除后同名重建时，Bookmark 同样可能解析成功并返回新对象。
- 这些样本未取得 Document ID；不能将两个 nil 当成身份相同，也不能从 nil 推断所有 APFS 文档均不支持该能力。

因此 Bookmark 的定位收益有实验依据，安全保存与任意替换的身份区分仍未闭合。

## 3. 原生适配器的最小边界与锁定依赖

适配器仅提供“创建定位数据”和“解析候选位置”能力，返回 Rust 值，例如 Bookmark 字节、候选本地路径及 stale。它不分配资源 UUID、不修改工作区、不授予权限、不自行决定合并或身份连续。

当前 [`Cargo.lock`](../../Cargo.lock) 已锁定并在本机缓存：

| crate | 锁定版本 | 可复用能力 |
|---|---|---|
| `objc2` | `0.6.5` | Objective-C 调用和对象生命周期 |
| `objc2-foundation` | `0.3.2` | NSURL、NSData、NSString、NSError |

已对照该版本本机 registry 的 `src/generated/NSURL.rs` 与 `src/data.rs`，确认存在：

- `NSURL::fileURLWithPath_isDirectory`。
- `bookmarkDataWithOptions_includingResourceValuesForKeys_relativeToURL_error`。
- `URLByResolvingBookmarkData_options_relativeToURL_bookmarkDataIsStale_error`。
- `NSURLBookmarkResolutionOptions::{WithoutUI, WithoutMounting}`。
- `NSData::{with_bytes, to_vec}`。

创建使用普通 Bookmark，解析沿用 spike 的 `WithoutUI | WithoutMounting`，不弹系统定位 UI、不主动挂载卷。只接受本地 file URL，检查对象类型，再用 Rust 路径和 metadata 复核。stale 不等于身份不连续，也不等于身份连续。

Core 当前未直接声明上述 crate；若后续选 Rust binding，需在 Core 增加 macOS target 的同版本直接依赖声明，并按实际调用启用 `std/NSArray/NSData/NSError/NSString/NSURL` 等必要 feature。不能直接 `use` Tauri 的间接依赖，也无需为此升级或重新选择版本。本轮没有执行任何依赖变更。实现时还须正确处理 autorelease、Retained、NSError 和 stale 指针生命周期。

普通 Bookmark 不是 security-scoped 访问凭据。本增量只服务当前已验证的运行方式，不据此声称 sandbox 持久目录权限、签名/公证或正式发行完成。原生调用放后台线程；`WithoutUI/WithoutMounting` 也不是严格耗时上界证明。

## 4. 单文件 locator：可独立实现的第一增量

### 4.1 私有持久数据及迁移

建议在正常 schema migration 中增加私有 `file_locators`：

- `resource_id`：主键，引用 `resources.id`，删除资源时级联删除。
- `identity`：创建该 Bookmark 时已复验的物理身份，用于防止同 UUID 重新指向后误用旧定位数据。
- `bookmark`：普通 Bookmark BLOB，不进入前端 DTO、localStorage 或源目录。

不要把 Bookmark 作为唯一事实源或资源主键。新旧 schema 升降级和备份策略须随 migration 明确；当前 v3 基线的下一版可为 v4，实际编号以实施时仓库为准。

旧文件已有 `resources.identity`，可以在当前位置实际对象仍与之相同时补建 Bookmark。捕获前后复核 identity，提交时再核对资源未被重新指向。不允许旧路径已换对象时补建定位数据并追认连续。

优先为活跃单文件授权、工作区引用和收藏捕获定位数据。不要默认对所有大规模扫描结果执行 Foundation 创建操作；捕获失败的支持范围和反馈需明确，不能静默声称定位增强已成功。

### 4.2 恢复流程

1. 读取资源、当前 identity、locator 及授权快照。
2. 先检查当前路径；它仍是原物理对象时无需为每次打开都解析 Bookmark。需要解析时，在 SQLite 写事务之外取得候选。
3. 候选必须是实际普通文件，规范化后读取 identity；它必须同时等于 locator 绑定 identity 和当前 `resources.identity`。
4. 进入短提交事务，重新核对资源、locator、授权及版本未变化，提交前再次检查候选对象。
5. 单文件 grant 仍活跃时，可以跟随同一物理对象；只有目录授权的资源，必须落在当前有效 root 且不在排除范围内。Bookmark 本身不能新增或复活 file grant。
6. 复用既有归一和投影路径更新名称、缓存路径、metadata、file_roots 与搜索投影；保留 UUID、工作区和收藏。实际 mtime/size 变化仍按现有事件规则处理，单纯恢复路径不伪造首次发现或修改。
7. stale 时，仅在上述身份和授权核验通过后刷新 Bookmark。身份改变、证据冲突、权限失效或解析失败时不换绑，保留关系及人工重新指向入口。

重新指向与合并要在同一事务替换/删除旧 locator。新对象即使沿用保留 UUID，也不能使用旧对象的 Bookmark 或单文件授权。副本不能继承原 locator。

### 4.3 打开与读取边界

当前 `resolve_resource` 持有 `gate.read`，不能在其内部直接升级申请 `gate.write`。应明确分离“候选捕获 → 短事务修复 → 正常读取/打开复验”，避免 RwLock 升级死锁。

恢复成功也不消除文件系统复核到按路径交给 OS 之间的竞争窗口。继续保留原有打开类型检查与系统接受请求后才记 opened 的语义。

## 5. Root locator：必须整体处理的第二增量

### 5.1 锚点、迁移及授权状态

建议增加私有 `root_locators`，至少保存 `root_id` 外键、原物理 `identity`、可空 Bookmark、定位有效状态。状态用于区别“用户授权意图仍保留”和“当前 root 路径可作为有效授权范围”，不借用 `scan_state` 充当权限事实。具体状态命名和暴露文案由实现决定。

**旧 root 没有原始物理 identity。迁移不能将旧 path 当前 stat 得到的目录自动认作历史授权对象。** 旧 root 无 locator 记录，不能承诺自动恢复；用户经原生选择器重新确认后，捕获目录 identity 与 Bookmark，并在提交前复验。旧记录的既有路径行为与新增锚点支持必须在状态/支持范围上区分，不能宣称已追溯验证旧授权。

新 root 捕获 Bookmark 失败时，仍可保存已确认的物理锚点，但须明确 locator 不可用，不能标成具备自动恢复。普通 Bookmark 依然不替代 OS 权限。

### 5.2 不只校验恢复目标，也校验旧路径

当前 `scan_root` 通过 `WalkDir` 枚举已存 root 路径。新增 root 身份语义后，旧路径可能已被另一目录占用；若只在 Bookmark 恢复分支比对 identity，普通扫描仍可能把新目录当旧授权根。

因此必须一并修改/复核：

- 扫描 setup、批次提交及恢复后的 root/generation 核验。
- `path_allowed` 和 `VISIBLE` 的有效 root 范围。
- `import_one`、`relink_impl` 为资源写入 file_roots 的归属判定。
- root 位置失效、恢复时的权限版本和 revision。
- 启动及关键授权使用前的 root 有效性核验，不能跨进程盲信上次验证状态。

root 无法确认时，保留记录、关联和用户授权意图，但挂起该 root 的有效 scope；其他有效 root 和 file grant 继续按并集提供授权。恢复只接受与原 root identity 相同的目录，不以同路径新目录替换锚点。

### 5.3 恢复提交与子资源

找到原 root 后，在同一短事务内更新 root 路径/有效状态、增加 generation、递增 permission_version 与 revision，使旧扫描和旧分页失效，再让活跃且未暂停的 root 从新路径扫描。保持 root UUID、相对 exclusions、暂停及撤销语义。

禁止按路径前缀直接改写全部资源 location。新位置的同名子文件可能已被替换。可以：

- 对活跃未暂停 root 复用正式扫描，逐文件按现有 identity 归一。
- 对已有重要引用使用自己的 file locator。
- 对暂停 root 的已有数据库记录，以旧相对路径计算新候选，分批逐项复验原 identity 后恢复；不枚举新增文件，不解除暂停。

短期恢复过程中应允许既有引用处于明确待核对状态，不以未确认的字符串改写维持表面可用。新路径若与另一条 root 记录的 UNIQUE(path) 冲突，明确失败并让用户处理，不静默合并不同 root 的排除、暂停和授权。

### 5.4 暂停、撤销、排除及备份

| 条件 | 必须保持的行为 |
|---|---|
| paused | 可核验并恢复同一 root 的位置；仍暂停扫描，不触发全目录枚举 |
| revoked | 不自动恢复成 active；locator 不能复活已撤销授权 |
| exclusions | 相对规则跟随同一 root；任何单文件候选仍重新检查当前排除及独立 file grant |
| 重叠 roots | 独立验证每份授权；旧成员关系不能授权已经移出其有效范围的资源 |
| 备份恢复 | 延续当前撤销全部 scan_roots/file_grants 的策略；定位元数据不是恢复授权的理由 |
| 根离线/身份冲突 | 不扫描替代目录、不根据失败推断用户删除原件、不删除已有关系 |

## 6. 状态所有权、锁顺序与受影响消费者

Rust 继续拥有资源 UUID、身份、授权和持久定位数据。React 不保存第二套 Bookmark、候选路径或授权事实。

Foundation 解析不放在 SQLite 长写事务或 UI 线程内。先读快照、外部解析，再短事务核对原 identity、原位置、locator、active 状态、generation 和相关版本；解析期间 revoke/exclude/relink/restore 发生时，旧候选必须失效。最终文件系统复验沿现有提交边界执行，不能将跨系统调用称为完全原子。

当前 Catalog 的顺序为 `catalog_write → gate.read → BEGIN IMMEDIATE`；root setup、授权变更和恢复使用 `gate.write`。不得新增持有 `gate.write` 后再等待 `catalog_write` 的反向依赖。root 恢复递增 generation，恢复数据库仍由既有 restore epoch 作废旧扫描。

| 消费者 | 必要处理 |
|---|---|
| `grant_root` | 选择时建立目录身份锚点；处理当前路径与已有 root 的冲突 |
| `scan_all/scan_root/commit_batch` | root 恢复顺序、身份/代次核验、不扫描替代对象 |
| `path_allowed/VISIBLE` | 共享有效 root 范围与排除规则 |
| `refresh_file_grants/resolve_resource` | 单文件定位恢复、状态及打开前复验 |
| `import_one/relink_impl` | 复用 UUID/归属，原 locator 替换、合并和授权隔离 |
| `start_watcher` | root/file 新路径注册与旧路径解除；失败沿现有明确反馈/重试规则 |
| `restore_from` | 新表迁移、旧授权撤销、版本递增及旧候选失效 |
| 查询与 UI 失效 | 路径/有效范围变化后版本更新，清除旧分页和旧读取结果 |

不新增静默按名称匹配、全盘搜寻、父目录授权、无界后台重试或另一套前端恢复状态。

## 7. 实施前后必须保留的回归

以下是下一轮待执行要求，本研究未执行，不应勾选为通过。

### 单文件与原生 API

- 普通 Bookmark 创建、立即解析、新进程解析、stale 刷新及损坏 BLOB 明确失败。
- 独立 file grant 的同卷改名/移动后，同 UUID、工作区、收藏、正确内容保持；邻居文件不获得授权。
- 删除后同名重建、原子替换、复制、另存为、symlink/非文件候选不得自动继承旧关系。
- 只有目录授权的资源移出范围拒绝；具有独立有效 file grant 的同一对象按该 grant 处理；revoke 后不复活。
- 解析期间 revoke/exclude/relink/restore、候选捕获后对象替换、合并确认期间替换均拒绝旧结果。
- 重新指向/合并后 locator 与保留 UUID 的新 identity 一致，副本和重复记录不遗留可复用旧 locator。

### Root 与跨层授权

- 新 root 身份锚点；旧 v3 root 迁移不自动追认历史对象；原生重新确认后建立锚点。
- root 改名、移动、旧路径不存在、旧路径被新目录占用；只恢复原 identity，不扫描替代目录。
- root 活跃、暂停、撤销，relative exclusions 与重叠 root 并集均保持。
- root 移位同时子文件替换、子文件另行改名，逐项核对而非前缀批改。
- 旧扫描已捕获批次后发生 root 恢复，新 generation/restore epoch 拒绝旧批次。
- 新路径与另一 root 记录冲突明确失败；无静默授权合并。
- 目录内资源、单文件授权、收藏、工作区、全局搜索、时间线计数、占位和打开使用一致有效范围。
- watcher 迁移路径、启动恢复、离线失败、备份恢复撤权及核心进程重开。
- root 大量已有引用的恢复分批、锁等待和查询延迟；不能以小样本正确性替代规模验证。

### 产品与证据退出条件

按 [U8](../../PRD.md#u8编辑保存后跨日继续工作) 在正式 WorkTwill 窗口完成两个工作区及收藏、实际编辑器保存、应用退出期间保存、重启取回，核对正确原件、UUID、关系与资源数量。Core 重开、API 成功、Chrome mock 和 Swift 新进程解析分别登记，不替代原生闭环。

物理 identity 改变的 Office 安全保存继续作为独立未覆盖项：需实际编辑器、文件系统、保存方式及可用原生文档延续证据。Document ID 不可用时不能通过降低防误连标准完成验收；需要反复手动修复的保存场景不算自动恢复通过。Windows、跨卷、云目录、系统重启及签名/sandbox 包另行验收。
