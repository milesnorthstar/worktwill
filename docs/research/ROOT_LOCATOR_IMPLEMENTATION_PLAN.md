# Root locator：下一轮完整实施计划

日期：2026-11-20。状态：023冻结时的研究与实施计划。024已按此接入实现；实际结果见[决定0022](../decisions/0022-root-native-locator.md)和[验收记录](../ACCEPTANCE.md)的024段。本计划中的“待执行”保留当时语境，不能代替后续实测记录。

本轮先冻结已完成的 023 UI 摘要。Root locator 留到下一实施轮完整处理，不将“增加表、解析 Bookmark、改写根路径”单独登记为完成。本次仅新增本文，不修改运行时、依赖或其他文档，不执行构建、测试、GUI 或基准。

本计划承接 [macOS locator 边界研究](MACOS_LOCATOR_BOUNDARIES.md)，产品行为仍以 [PRD](../../PRD.md) 为准，权限、事实源和验收边界以 [ARCHITECTURE](../../ARCHITECTURE.md) 为准。下文描述的是拟采用方向，不能读作当前已支持。

## 1. 目标与不变量

目标是在同一物理目录被外部改名或同卷移动后，恢复其已授权 root 的位置，保留 root UUID、相对排除规则和既有资源引用。Bookmark 只给出位置候选，目录与文件均须重新核对原物理 identity。

- Rust 继续拥有资源/root UUID、授权、物理 identity、持久 locator 与业务版本；React 不维护第二份授权或原生定位数据。
- 根授权与根定位有效性分开：`active` 表达用户授权意图；anchor 的 `valid` 表达当前路径能否作为有效范围。`valid=1` 本身不能激活 `active=0`。
- 同路径、同名、mtime、大小和 Bookmark 解析成功均不是身份连续证明；aliases 只用于构造候选。
- 不按旧路径前缀批量改写资源路径，不授予父目录或新增单文件 grant，不写源文件内容、元数据、xattr 或 sidecar。
- 不把 root 恢复扩成跨卷、Document ID 或 Office 物理替换支持。系统重启、云目录与正式签名/sandbox 包仍须单独验证。

## 2. 当前基线与拟新增数据

当前基线已有单文件 Bookmark 适配器与领域集成，schema 为 v5。Root 仍只有路径和原有扫描状态，没有持久身份锚点或 root Bookmark；相关源码见第 8 节。既有单文件适配器和测试不能代替目录的身份、权限或规模验收。

拟通过正常 v5→v6 migration 增加：

| 私有结构 | 最小字段/约束 | 含义 |
|---|---|---|
| `root_locators` | `root_id` 为 FK/PK，`identity` 非空，`bookmark` 可空，`valid` 为受约束布尔值 | 目录原始物理身份、普通 Bookmark 与当前定位有效状态 |
| `root_aliases` | `root_id` 外键、历史 `path`，二者唯一 | 原 root 的历史前缀，仅构造已有资源的位置候选 |

建议外键删除使用级联；实际 migration、必要列校验与失败回滚必须一起实现。Bookmark 和 anchor 不进入前端 DTO；UI 可以获得必要的定位状态，但不能获得原始 BLOB。

### 2.1 Legacy 不追认历史身份

旧 root 没有原身份可比较。迁移保持 locator 缺失，沿用原有路径语义并明确标为 legacy。不能在 migration、启动或后台扫描中对旧路径 stat 一次，就将其当前目录追认为历史授权对象。

只有用户通过原生选择器明确重选目录后，才能建立锚点。迁移也不能给 legacy root 生成看似有效的历史 aliases。Legacy 行为与新 anchor 保证必须在支持范围和状态说明中分开。

## 3. 新授权与明确重选

`grant_root` 在 Store gate/SQLite 写事务之外，捕获规范化目录路径、实际目录 identity 和普通 Bookmark；提交前复验对象身份与目录类型。

- 新授权绑定实际 identity。Bookmark 不可用须明确反馈定位增强的限制，不伪造有效 Bookmark。
- 明确重选与现有 anchor identity 相同的目录时，优先复用原 root UUID 和相对 exclusions，而非按新 path 建立第二个 root。显式授权是否恢复扫描沿用既有产品语义，不由后台恢复擅自决定。
- 新路径已属于另一条 root 时，明确冲突；不静默合并不同 root 的暂停、排除或授权。
- 明确重选不同 identity 的目录，属于新的授权决定；替换 anchor 时清除旧 aliases，不把旧 root 的历史位置解释成新对象的延续。
- 任何恢复均不得激活已撤销 root。用户重新选择后的重新授权与自动恢复必须保持不同入口和证据。

## 4. 启动核验与后台恢复

### 4.1 Store::open 只做 metadata 核验

采用方向为：启动只核验已有 anchor 的原路径 metadata，不调用 Foundation，不枚举目录。原路径仍为原目录时保留或恢复 `valid=1`，避免正常目录每次重开都先从搜索消失；原路径丢失、类型变化或 identity 不符时标为无效。

`valid` 的变化与必要的 generation、permission_version、revision 更新在同一事务完成。只要有效授权范围改变，就必须递增权限版本和 revision；不能只等后台恢复成功后才递增。未变化的正常根不应为核验本身不断制造业务版本变化。

Legacy root 不在此阶段建立锚点。已撤销 root 不因 metadata 成功而重新获得有效授权。

### 4.2 refresh_roots 与按需恢复

丢失或移位的 anchored root 由后台 `refresh_roots` 或打开旧引用时按需恢复：

1. 在读取快照中取得 root UUID、原路径、原 anchor identity/Bookmark/valid、generation、授权与版本。
2. 在 gate 与 SQLite 写事务之外解析普通 Bookmark；不弹 UI、不主动挂载卷。
3. 候选必须是本地目录，canonicalize 后与原 identity 相同。原路径被另一目录占用，不构成恢复依据。
4. 提交时 CAS 核对原 anchor、原路径、generation、active、相关授权版本及其他参与快照的条件；再次核验候选目录。
5. 同事务保存旧 path 到 aliases，更新当前 path、Bookmark/valid，增加 generation，按有效范围变化递增权限版本和 revision。
6. 活跃未暂停根随后通过正式扫描归一文件；暂停根仅核对已有资源，不能启动新增枚举。

定位失败仍保留 root、工作区和资源关系。标记根失效的事务必须真正提交；不能只在随后返回错误的事务里写 `valid=0`，让回滚继续保留旧有效范围。

单根定位错误与数据库/锁错误分别处理：前者允许其他根完成后明确报告，后者及时失败。错误只在当前快照仍适用时收集，避免对象已被修复或撤权后返回过时定位错误。

## 5. Aliases 与已有资源恢复

`root_aliases` 持久化历史旧前缀，使暂停根在移动、进程退出或中断后，仍能找到已有资源的相对位置候选。保存 alias 与 root 路径更新必须同事务完成。

候选构造使用路径组件语义，例如 `Path::strip_prefix`，不使用不带边界的字符串前缀。候选 join 后必须 canonicalize，检查当前有效 root 范围、当前 exclusion 和原资源 identity；symlink 导致越界时拒绝。路径中的 `%`、`_` 等按字面处理。

Aliases 不得进入 `VISIBLE` 或 `path_allowed` 作为额外授权范围。旧目录被新对象占用时，其名称相同也不能承接 root 或文件身份。

### 5.1 未暂停与暂停根

- **未暂停根**：正常扫描当前 root，由原 identity 复用资源 UUID；不前缀批改。
- **暂停根**：只遍历已经存在的数据库资源映射，逐一 stat 候选，核身份后更新；不枚举目录中的新增文件，不解除 paused。
- 暂停根大量已有资源的恢复需要有限批次和中断检查。实现遍历时避免在“旧位置”条件不断被更新移除的集合上使用递增 OFFSET 而漏项；应使用稳定键或已固定的批次集合，并在每批提交时重核快照。
- 资源仍无法确认时保留引用和待核对状态；不以猜测更新保持表面可用。

### 5.2 RootRecovery 与原 ingest 事务

拟复用原 `import_one_checked` 提交路径，可采用私有 `RecoveryGuard` trait，也可用等价的受控内部类型。它只服务已经存在的 File/Root 两类恢复，不是对外 DTO、IPC 参数或新授权 API。

| Guard | authority | 对文件 locator 的更新 |
|---|---|---|
| File | 原资源/identity/location、活跃 file grant、原 file Bookmark、版本 | 可以提供该文件的 `updated_bookmark` |
| Root | 原资源/identity/location、原 root/anchor/path/generation、有效当前 root 范围、exclusions、版本 | `updated_bookmark=None`，不得把 root BLOB 写入 `file_locators` |

导入入口本身必须核候选 identity 等于 guard 的原文件 identity，不能只依赖调用者另传的 `expected_identity`。Root 分支不能冒用 FileRecovery 的 grant 条件；`grant_selected` 保持 false。提交时分别复验各自 authority，不新增授权。

## 6. 有效范围、状态与监听

### 6.1 统一范围与暂停/撤销

有效范围规则必须同时被 SQL 可见性、原生打开前权限检查和归属写入使用：

- Legacy：保留已披露的旧路径语义，不声称原生身份锚定。
- Anchored：要求 active、valid；实际授权/扫描前还要核对目录 metadata 与 anchor，禁止替代目录。
- Paused：保留授权意图和当前有效范围，只停止新增扫描；可以恢复位置和逐项核对已有资源。
- Revoked：不恢复，不激活；aliases 和 Bookmark 都不能改变撤销结果。
- Excluded：相对规则跟随同一 root。候选仍按当前 exclusion 复核；独立 file grant 的并集语义维持原规则。

仅修改 `path_allowed` 不够。导入和重新指向当前各有手工 `roots_db` 过滤循环，若继续仅检查 `active + path prefix`，仍会向失效根写入错误归属。

### 6.2 打开旧引用的入口

根已经恢复后，旧 `resource.location` 不再匹配当前 root 路径，现有 `resolve_resource` 可能先被 `VISIBLE` 拒绝并返回 `not_authorized`。RootRecovery 必须处理这种“旧位置不可见”的内部候选入口，不能只接文件不存在或身份变化错误。

允许按 UUID 读取原资源事实作候选查询，不等于授予旧路径访问。最终在原 ingest 事务内按新有效 root、当前排除和原 physical identity 授权；无当前有效授权时拒绝。避免在持有读 gate 的 resolve 内升级写锁。

### 6.3 UI 与 watcher 仍须完成的设计

UI 要能区别 legacy、定位有效、位置待核对和授权已撤销；暂停文案不能掩盖根位置无效。传递必要的 Rust 状态即可，不暴露原始 Bookmark，不在 React 创建恢复事实源。

Watcher 只监有效且符合暂停规则的根，路径更新后解除旧路径、注册新路径。单文件 grant 继续保持独立监控和授权边界。

无效根不再被 watch 后，自身不会产生新事件。因此必须明确启动、显式 rescan、open 按需恢复以及空 watch 集合时的恢复触发。是否采用受预算约束的 metadata 重核/退避，要在实施中确定并验证；不能只等待一个已不存在的事件源，也不能无界快速重试 Foundation 或反复报错。

## 7. 锁、扫描收尾与恢复安全

Foundation 在 gate/SQLite 写事务之外调用。当前 Catalog 顺序为 `catalog_write → gate.read → BEGIN IMMEDIATE`；不得新增持有 `gate.write` 再等待 `catalog_write` 的反向锁顺序。

根恢复增加 generation，作废旧枚举和旧批次。`scan_root` 的 setup、扫描中检查、commit 和 finalize 均须使用新有效性条件，并实际核验原目录身份。数据库恢复仍通过现有 restore epoch 拒绝恢复前批次。

扫描 finalize 中“另一 active root 的当前 generation 防止标记 missing”的查询也要排除无效 anchored root，检查有效范围；不能让失效根的历史归属继续保护陈旧可用状态。身份不明、取消、离线或部分失败仍不能当成完整枚举后的删除证明。

备份恢复必须在写回当前库前显式校验 `root_locators` 和 `root_aliases` 的必要列，延续 022 对必需表的检查原则。旧备份经 migration 建表，但不生成历史锚点；新备份缺必需表/列明确拒绝。恢复继续撤销 root/file grants，定位 valid 不得复活授权；建议一并失效恢复后的验证状态。

## 8. 必须接入的源码消费者

以下按当前符号定位，实施时重新核查，不仅修改列表中第一处。

| 文件/符号 | 必需工作 |
|---|---|
| [`migrations.rs`](../../crates/worktwill-core/src/migrations.rs) | schema6 两表、外键/约束、回滚与 legacy 迁移 |
| [`store.rs`](../../crates/worktwill-core/src/store.rs) `Store::open` | 仅原路径 metadata 核验；有效范围/版本原子更新 |
| 同文件 `grant_root` | 锁外捕获、提交复验、按 identity 重选、UUID/exclusion 保留、冲突与 aliases 清理 |
| 同文件 `VISIBLE` 两平台分支、`path_allowed` | legacy 与有效 anchor 条件一致，aliases 不授权 |
| 同文件 `RevokeRoot/SetPaused/Exclude/Include` | 作废旧候选；暂停、撤销和相对排除语义 |
| 同文件 `resolve_resource` | not_authorized 的受控根候选入口、单次恢复及正常打开复验 |
| 同文件 `relink_impl` | 有效 root 归属，旧 locator/alias 的身份边界，不借重新指向扩大范围 |
| 同文件 `restore_from` | 新必需表/列校验、撤权、验证状态与旧候选失效 |
| [`catalog.rs`](../../crates/worktwill-core/src/catalog.rs) `scan_all` | refresh_roots 顺序，包含 paused，单根失败不阻塞无关根 |
| 同文件 `scan_root/scan_valid/valid/commit_batch/finalize` | 实际锚点、代次、取消与失效收尾；另一 root 的 missing 保护查询 |
| [`ingest.rs`](../../crates/worktwill-core/src/ingest.rs) `needs_file_grant/import_one_checked` | 当前有效范围、RecoveryGuard、归属写入、投影与版本 |
| [`file_locator.rs`](../../crates/worktwill-core/src/file_locator.rs) | 保持单文件独立 authority，必要时复用私有 guard，不写错 Bookmark 类型 |
| [`macos_bookmark.rs`](../../crates/worktwill-core/src/macos_bookmark.rs) | 复用普通原生适配器；新增目录实际 API 验证，不将候选当身份 |
| [`main.rs`](../../src-tauri/src/main.rs) `start_watcher/kick_scan/open_resource/reveal_resource` | 有效监听源、恢复触发、目录失败反馈及定位写入后的 UI 失效通知 |
| [`model.rs`](../../crates/worktwill-core/src/model.rs)、生成 contracts、[`App.tsx`](../../desktop/src/App.tsx) | 必要状态 DTO 与中英文 UI；不暴露原生数据，不创建第二授权源 |
| 查询/日历缓存/分页消费者 | scope 或路径变化带权限/revision，旧结果、缓存和游标一致失效 |

## 9. 下一轮必须执行的回归与验收

以下全部为待执行清单，本文没有将任何一项登记为 root locator 已通过。

### 迁移、授权及重开

- 真实 v5→v6 migration 原子成功/失败回滚；旧 root 无 anchor/alias，不 stat 追认；旧 root 明确显示 legacy。
- 新授权捕获目录锚点；同 identity 明确重选复用 root UUID 与相对 exclusions；不同 identity 清 aliases；路径另属 root 明确冲突。
- 正常存在且 identity 不变的根重开后有效、搜索不先丢失；启动不调用 Foundation、不枚举目录。
- 启动发现原路径丢失、替代对象或目录类型变化时，有效状态与权限/revision 同事务变化；日历及旧游标不能沿旧版本复用。
- schema6 备份缺新表/必要列在写回前拒绝；旧备份迁移后仍需明确重新授权；restore 不复活 anchor 权限。

### 目录、aliases 与暂停

- 实际目录改名、同卷移动、父目录改名；原 root UUID、已知文件 UUID、工作区、收藏和当前排除保持。
- 同一 root 连续多次移动，跨进程/中断后 aliases 仍可构造旧引用候选；原路径被另一目录占用时不扫描替代目录。
- Paused 根移动后重开，只核对已知数据库映射；新加入磁盘的文件不被枚举或创建资源。
- Paused 大量既有映射分批恢复无漏项，取消/撤权/代次变化停止旧批次，不用变动集合上的 OFFSET 跳过剩余记录。
- 子文件替换、子文件另行改名、复制、删除后同名重建、symlink 越界、嵌套 aliases 及字面特殊路径均独立核身份与 scope。
- 旧路径不可见的工作区引用可走 RootRecovery，授权失败不能绕过；不新增 file grant。

### 并发与失败

- 捕获候选后 revoke/pause/exclude/include/relink/regrant/restore，旧 guard 正确失效；File/Root authority 互不冒用。
- 目录在解析和提交之间被替换、根路径与另一 root 冲突，拒绝错误提交并保留旧关系。
- 旧扫描已枚举批次后 root 恢复/失效，setup、commit、finalize 均不覆盖新事实，不把失效另一 root 当有效保护。
- 单根 Bookmark/离线错误仍扫描无关根后明确返回；数据库/锁错误及时失败；已修好/已撤权对象不报告过时错误。
- 定位已写入但系统 open/reveal 失败，UI 仍读取新路径；opened 只在系统接受后记录。
- 空 watch 集合、paused 根及介质重新出现后，按已明确策略触发恢复，不依赖失去的 watcher，不形成无界重试或通知洪峰。

### 产品与性能证据

- UI 区分 legacy、暂停、定位不可用、已撤权，中文/英文和失败后的恢复入口均可理解。
- 根恢复、首次扫描与 paused 已知映射 stat 分别报告耗时/锁等待；不能以“没有枚举新增文件”推断 I/O 成本很低。
- 原生选择器、真实目录变更、正式窗口查询/打开、退出重启分别验收；Core、原生 API、浏览器 mock 和 GUI 证据不互相提升。
- F09/U8 的 Office 原位保存与物理替换保存继续单列。本计划不解决 identity 改变后的文档连续性，也不以交付时间降低防误连或真实原生验收标准。

## 037实施补充

指定目录维护现由Core按当前暂停状态选择已有映射核对或正式扫描，共用既有100条稳定UUID分页；Runtime复用036有界队列。文件位置回写不再要求无关全局revision相等，准确的目录/权限/资源和物理身份检查保留；目录解析CAS仍检查revision，查询与恢复版本检查未变。部分暂停在线移位、恢复扫描、撤权和重授权已取得实际原生证据，未完成矩阵保持，见[决定0034](../decisions/0034-paused-root-maintenance.md)。
