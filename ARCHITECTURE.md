# WorkTwill V1 技术架构与搜索性能基线

2026-11-21 安装更新增量：Rust壳层新增更新服务；React只展示带revision的状态，便签仍由原组件串行保存。Tauri插件负责验签与安装，官网与更新清单共用逐平台发布记录，业务SQLite所有权不变。见[安装和更新](docs/INSTALL_AND_UPDATES.md)。

**v0.6 · 2026-11-19 · V1 实现与验收基线**  
文件身份、原生拖入、日历时间线、标题联网行为、跨平台分发和端到端搜索性能仍须对应环境验收。产品路径和交互契约见 [PRD](PRD.md)。

当前正式工程在 `desktop/`、`src-tauri/` 与 `crates/worktwill-core/`，当前 schema v9；已增加原生 token 拖入/单文件授权、可关闭的网页标题获取、日历文件时间线与完整搜索分页代码。浏览器原型和独立实验保持独立。以下是完整实施契约，不是全部已完成能力。实现与验收缺口见 [开发状态](docs/DEVELOPMENT_STATUS.md)，当前工程选择见 [原生开发基线](docs/decisions/0001-native-development-baseline.md)。已确定的技术栈与领域边界保持不变；文中尚未实施的索引优化、原生文档身份和发行方式须通过验证后记录决定。[文件跟踪调研](FILE_TRACKING_RESEARCH.md) 提供 API 与竞品依据，实际实验状态见 [实验结果](spikes/RESULTS.md)。AI 开发维护流程见第 12 节，与第 11 节未来产品 Agent 分开。

## 0. 核心不变量

1. 资源在应用内由**稳定 UUID** 标识；真实文件身份、缓存路径、URL 都是资源的可变属性，不作为工作区关系主键。
2. `Resource`、`Workspace`、关联关系、`ActivityEvent` 是长期稳定的领域概念；新增笔记/个人小面板时允许通过版本化 migration 扩展表和 UI，不承诺数据库零改动。
3. **搜索是最高优先级基础服务**：顶部全局常驻、查询可取消、离线即时响应、支持混合资源、可重建索引与完整基准测试。
4. 索引和搜索的派生数据不得充当唯一的业务事实来源；重建搜索索引不影响工作区关联、已保存网页或活动日志。
5. 不改动源文件、不读取本地文档正文、仅索引明确授权目录或单文件；离线核心不依赖常驻进程、外部账号和网络服务。网页标题是用户可关闭的联网增强，边界见 4.2。
6. 应用对源文件保留只读访问。跨区「移到」只变更关联表，目录授权仅改变应用可访问和索引的范围。
7. 文件资源表示经验证的引用连续性，不要求底层物理 File ID 永远不变；物理身份、文档延续身份、位置和访问凭据必须区分。没有充分身份依据时不按名称或路径自动换绑。

## 1. 已确定架构

- 桌面：**Tauri 2**，不将浏览器网页作为具备全盘原生操作能力的独立客户端。
- UI：Vite + React + TypeScript；左侧全局导航与右侧主内容，个人工作台为默认首页，全局搜索常驻右侧内容顶部；轻量分页列表，日历/时间线仅为动态页私有视图。资源数超过100时启用固定84px行高、最多16行的窗口渲染；实际缩放、长文本、滚动选择和规模性能仍需验收。
- 核心：Rust；将文件扫描、ID 获取、定位、权限检查、监听、搜索及数据库写入留在原生后端。
- 存储：rusqlite bundled SQLite，独立连接、WAL + synchronous=FULL、事务与显式 migration；当前 schema v9，数据库放系统应用数据目录。磁盘与并发故障验收仍单独执行。
- 搜索：统一资源搜索服务 + SQLite 索引视图；V1 查询文件名/路径、网页标题/URL、工作区名，不读取 Office/PDF 正文、不生成语义 embedding。
- 平台：Windows 和 macOS 适配器隔离，避免将 NTFS 特性误认为通用标准。
- 网络：本地核心无登录、AI 和永久后台服务依赖。新增网页可关闭自动标题：启用时访问目标网站并经固定 Cloudflare DoH 解析域名；关闭或失败仍可手填保存。没有网页脚本、浏览器 cookie、远端图标或正文持久化依赖。

## 2. 参考分层

```text
React Workbench (左侧导航 / 个人工作台默认首页 / 右侧顶部搜索 / 工作区 / 动态 / 收藏)
        │ typed Tauri commands + events
Rust Application API (path checks, orchestration, transactions)
   ├── RootPermissionService  目录与单文件授权/排除/撤销
   ├── FileCatalogService     扫描/元数据/批量更新/去重
   ├── FileLocator            稳定引用、路径修复、失效状态
   ├── ResourceService        通用资源/文件与网页适配
   ├── WorkspaceService       工作区与多对多关系
   ├── SearchService          混合搜索、排序与取消、可重建投影
   ├── ActivityService        双时间文件观察、日历聚合与次级应用事件
   ├── WebMetadataFetcher  有界公网标题获取、独立于离线搜索
   └── PlatformAdapter
       ├── Windows: File ID / ReadDirectoryChangesW / 评估已有 Object ID
       └── macOS: NSURL Bookmark / FSEvents / 评估 Document ID
        │
SQLite (业务表 + 搜索索引表 + schema migrations)
```

Rust 和 TS 的 API 必须使用明确的参数/返回类型；不要把可执行的任意 SQL 或任意路径写命令暴露到前端。

### 2.1 事实源、状态所有者和消费者

| 事实/状态 | 唯一业务所有者 | 主要消费者与边界 |
|---|---|---|
| 目录/单文件授权、排除规则、授权版本 | RootPermissionService | 扫描、定位、资源读取、搜索、动态；前端选择状态不构成授权 |
| 文件发现、当前目录归属、扫描代次 | FileCatalogService | ResourceService、SearchService；监听器只提供观察事实，不直接跨服务写库 |
| 原生身份、候选位置及恢复证据 | FileLocator + PlatformAdapter | Catalog 归一与打开；不擅自创建工作区关系或授予权限 |
| 稳定资源 UUID、当前有效身份绑定 | ResourceService | Catalog、Workspace、Favorites、Activity；扫描和拖入复用同一归一操作 |
| 工作区、关联、引用转移 | WorkspaceService | UI 和未来工具；服务内事务，不由前端分两次“先加后删”实现 |
| 搜索投影、查询与重建状态 | SearchService | 全局搜索和工作区添加入口；不成为业务与授权事实源 |
| 应用行为与文件观察事件 | ActivityService | 工作台、动态；不把扫描 mtime 推断为逐次编辑历史 |

React 仅持有视图、输入、多选和当前请求等交互状态，通过命令结果与版本事件刷新业务状态；不在 localStorage 中维护第二份正式业务库。Rust 内部服务相互调用共享业务能力，不再经前端或 IPC 绕回。上述是逻辑职责，工程初始化不要求每项单独建 crate、进程或 store。

App 保留交互状态归属：`home` 对应默认「个人工作台」，`favorites` 为固定读取收藏资料的独立视图，不再借用 `files` 及可切换模式；`files` 页面状态、所有文件导航及返回文案已移除。左侧导航按个人工作台、工作区、动态、收藏排列。首页复用现有资源查询与列表面板，只提供最近修改/最近打开，工作区卡片读取当前概览中的前3项，继续以原工作区及资源UUID导航、关联、打开；不增加「上次工作区」持久状态，不将排序数据伪装成访问历史。全局搜索与完整结果的来源上下文继续由 App 管理，收藏返回由视图固定恢复收藏查询；日历、时间线、局部筛选只在动态视图内部使用。本次删除不改 Rust 查询、授权、DTO 或数据表，空词仅进入完整页而不执行搜索，不新增旧视图兼容层；见[决定0037](docs/decisions/0037-remove-all-files-page.md)。

026的最近使用查询以当前文件identity与opened观察来源匹配，筛选和排序共用条件；网页不强制文件origin。选择器返回后先固定整批ImportSelection，再复用checked导入。正式原生打开使用Rust私有ResolvedOpen，prepare在同一读事务冻结对象/位置/名称/权限/session/恢复epoch，系统接受后用短事务重新核身份和授权才记录观察；不以完整revision拒绝无关变更，不跨系统调用持锁。变化明确报告已接受但活动未记录。schema及DTO未变，按路径打开的OS时间窗口仍存在，详见[决定0026](docs/decisions/0026-current-object-recent-and-open.md)。

侧栏宽度随可用空间收为图标栏，保留按钮名称与当前页语义；窄宽下可以隐藏工作区快捷列表，不能隐藏工作区主入口。固定侧栏使用 `overflow-y: auto`，主导航与底部设置入口不能因矮窗口或放大被裁掉。首次空库的主要操作是明确授权管理位置，仍复用现有授权命令、范围就绪判断与空态，不引入第二套引导状态。详见[决定0025](docs/decisions/0025-personal-workbench-sidebar.md)。

2026-11-21 UI参考对齐：`ResourcePresentation` 统一文件类型图标、大小、时间和字面高亮；高亮只消费Rust返回的 `match_field`，工作区标签从当前overview按UUID解析，不新增事实源或查询。首页最近资料使用252px内部滚动区，保留已加载资料、选择和分页，虚拟列表仍保持84px固定行高；宽度不足时小组件移到下方。快捷搜索固定在视口内，只有结果区滚动；完整结果按可用宽度隐藏大小/工作区列。复选框与下拉框统一外观，保留原生目录及日期选择能力。对应[本轮验证记录](docs/validation/2026-11-21-ui-reference-alignment.md)，不替代Windows原生验收。

### 2.2 当前 V1 命令和事件契约

- Rust DTO/错误定义为跨端契约事实源，当前使用 ts-rs 从 `model.rs` 生成 `desktop/src/contracts.ts`；`pnpm types:check` 检测生成结果漂移。完整目标接口与当前增量的差异见开发状态，不能把生成契约检查通过视为全部目标接口已实现。
- 查询命令覆盖资源/工作区/动态读取、搜索及扫描/重建进度；写命令覆盖授权管理、网页属性、收藏、工作区和关联、重新指向及备份恢复。命令名称以导出的唯一契约为准，不能让各页面各自定义一套。
- 普通资源操作使用 `resource_id`/`workspace_id`；前端传入路径不能作为身份或授权证据。目录/文件授权和重新指向通过原生选择与确认流程取得候选，由 Rust 再核验；平台访问凭据不返回 WebView。
- 搜索请求携带 `request_id`、查询、范围、类型、分页参数；结果携带对应请求标识、授权版本、投影版本/状态及分页信息。取消和过期结果不得伪装成空结果。
- 关联、转移及重新指向等复合写操作携带 `operation_id`，处理结果与业务变更同事务留存，用于重试确认；同操作重放不能重复写活动、重复合并或删掉后续新建的关联。超时但提交结果未知时，先查询操作结果或重新读取状态，不盲目重复执行。
- 批量操作返回逐项 `added/already_exists/pending_authorization/failed` 等类型化结果，保留局部成功。来源/目标相同返回无变化。
- 最少区分 `invalid_input`、`not_authorized`、`permission_changed`、`resource_unavailable`、`identity_unconfirmed`、`conflict`、`cancelled`、`stale_cursor`、`index_unavailable`、`storage_full`、`database_busy` 和 `unsupported_data_version`。当前AppError强制携带code/message/message_en，ImportItem失败保留双语正文；UI保留原始错误，在显示时按当前语言选择，切语言不重试业务或网络请求，见[决定0010](docs/decisions/0010-bilingual-error-feedback.md)。普通插件/JavaScript异常保留原文，不承诺自动翻译。错误不可静默转成功，也不直接暴露底层路径/SQL。
- 业务事件在事务提交后发布，包含类型、受影响 UUID 和版本；扫描/重建进度另行限频。事件用于通知刷新，SQLite 才是业务事实源；事件丢失可通过重新读取恢复，不要求引入消息中间件。
- 038扫描批次仅在查询事实改变时递增revision，代次/进度/无变化收尾不递增；全量与指定目录完成前后版本相同只发scan-progress，前端复读概览并检查版本/读取代次，保留已加载搜索页。真实变化或版本读取失败仍走catalog-changed同步清旧资料；搜索与时间线游标的版本约束保持。详见[决定0035](docs/decisions/0035-catalog-query-stability.md)。
- 授权变更与版本递增同事务提交。启动建立新会话标识；当前恢复保留运行会话并将权限/revision提升至恢复前版本之上，前端拒收旧会话或旧授权版本结果，避免备份恢复后版本数字重复。009另外递增不从备份恢复的进程内扫描epoch，阻止旧枚举代次碰撞。

### 2.2.1 当前实际命令与目标差距

- `overview/resources/search/search_page/timeline` 为读取入口；结果提供 `session/permission_version/revision`。快速搜索取最多30项并以额外候选计算 has_more；完整搜索页每页100项，排序为 relevance/name/modified。完整搜索续页须回传版本化cursor，后端在同一读事务中核对条件/session/权限/revision，不匹配返回stale_cursor；前端清旧页后重读首页，不宣称跨写入保留旧快照。普通资源列表仍按offset和前端版本检查重读。取消和请求会话见决定0003，游标见决定0005；原生IPC端到端仍待验收。
- `mutate` 统一管理工作区/排序、file/web 关系、转移、收藏、网页保存/删除、目录/单文件撤权、排除和索引重建；operation_id 与普通业务写操作同事务落库。`SaveWeb.workspace_id` 在同一事务内建立目标引用；013的`CreateWorkspace.resource_ids`为可选字段，在创建事务内复用Link授权/去重逻辑，失败全部回滚；None省略序列化，保留普通创建的既有回执payload。
- `pick_root/pick_files/pick_relink` 使用原生选择器；pick_root可空suggestion仅允许desktop/downloads/documents，由Rust标准目录API设置选择器起点，只有最终选择结果可授权，见[决定0009](docs/decisions/0009-suggested-managed-locations.md)；`import_drop` 只接收 Rust 创建的 token、目标工作区和授权意图，不接收可授予权限的任意前端路径。`fetch_web_metadata` 只接收用户输入的网址，返回纯文本标题及最终 URL。
- `files-dropped` 通知含 token、数量和物理坐标；`catalog-changed` 当前只作为重新读取的信号，完整类型化领域事件/受影响 UUID/版本载荷仍是目标契约，不能将空刷新通知描述成已实现事件总线。
- `ImportResult` 逐项返回 `added/already_present/not_authorized/failed`。原生导入按项事务；UI 多选 Link/Transfer 目前按资源逐项调用正式 mutation，并汇总成功与失败，失败项保留来源。核心批量命令仍是整体事务，其他调用者不能将多资源单次调用当成逐项失败隔离；目标类型化批量结果也不等同前端汇总。
- 当前 `Resource.kind/status` 与导入结果 status 仍是字符串字段；DTO 生成检查不等同于已升级为所有目标 typed union。`WebMetadata` 已统一由 core/model.rs 定义并生成 TypeScript，原生命令与 api.ts 复用该类型。

### 2.3 状态与转换要求

| 状态域 | 状态及转换规则 |
|---|---|
| 定位 | `available/missing/permission_denied/volume_offline/identity_unconfirmed`；解析或事件触发复核，只有身份和授权通过才能回到 available |
| 授权 | 有效、暂停扫描、已撤销分别处理；暂停不撤权，离线不撤权；撤销后仅能通过明确授权流程恢复 |
| 扫描 | `idle/running/paused/cancelled/partial_failure/completed`；只有有效范围完整核对才能登记 completed，重启中的未完成任务不能当成完整快照 |
| 搜索投影 | `ready/rebuilding/failed`；重建成功校验后切换版本，失败保留业务数据并报告状态；重建期间是否可查旧投影须符合授权与新鲜度契约 |

`resources.status` 是服务按类型与定位/授权结果维护的摘要，不是前端可独立设置的事实；对文件以当前授权及 `locator_status` 判定可访问性，禁止两套状态各自演化。

### V1.3.1 数据与刷新增量

- `Resource.opened_at` 是可选的当前物理对象最后真实打开时间，不覆盖 `modified_at`；未授权占位清除它。`match_field` 由Rust按同一NFC/lowercase匹配返回 name/path/url，前端不猜测高亮或命中。
- `quality.rs` 仅识别 `.DS_Store`、`Thumbs.db`、`desktop.ini`、Office `~$` 锁文件，以及明确工具链目录段 `.git/node_modules/.cache/.next/.next-core-local/target/debug/target/release`。不概括全部隐藏文件、target业务目录或媒体。最近修改的默认推荐在LIMIT前应用，文件观察明细/日历/概览共用同一质量谓词；不删除任何记录。当前收藏/关系/打开豁免历史噪声必须属于同一观察identity；既有授权谓词独立生效。搜索不套用推荐过滤。
- 原生普通打开成功发送 `resource-opened`，App复验session/权限后后台重读当前已加载范围及新游标，按UUID保留仍有效选择。失败可能提交定位变化，仍发送 `catalog-changed` 强失效。打开不再复用成功后重复reload的通用run。扫描、撤权、身份与恢复强失效保留；无变化scan-progress继续只更新进度。
- `notes` 返回带进程session/恢复epoch scope的正式便签集合；`write_note(NoteWrite)` 使用预期版本、稳定UUID与墓碑，在同一短事务提交正文/置顶/删除。相同已提交快照重试返回回执，不重复创建；过时版本或恢复scope明确conflict。便签写不改资源revision、不入资源索引/活动，发送独立 `notes-changed`；恢复另发 `data-restored`。
- `scratch_notes(id PK,text,pinned,version,updated_at,deleted)` 在v8→v9事务创建，现有旧库无便签。备份包含此表，恢复暂存校验列及迁移；恢复epoch阻止旧待保存污染恢复库。Store打开较旧正式库前用Online Backup保留唯一命名的升级快照；失败不跳过迁移。发布回退须匹配旧程序与升级快照，不能让旧程序写schema9。
- `HomeWidgets` 持有便签编辑草稿和提交状态，按条串行发送最新快照；只有草稿等于已确认版本显示已保存。失败允许重试，冲突可明确保留为新便签。关闭窗口尝试提交，失败保留应用；强制退出不承诺未提交内存草稿恢复。组件一直挂载，页面/响应断点不丢草稿。
- 调试包仅在显式 `WORKTWILL_NATIVE_DATA_DIR` 位于 `WORKTWILL_TEST_ROOT` 的独立子目录时隔离原生验收数据，可选原生最小900×640及Web Inspector观测；release编译不读取这些选项。此机制用于测试，不宣称OS级隔离或新授权。

## 3. 当前持久模型与迁移（schema v9）

物理事实源为 `crates/worktwill-core/src/schema.sql` 和 `migrations.rs`。schema.sql 先建 v1，再依次执行 v1→v2（活动来源/文件时间、单文件授权）、v2→v3（工作区排序）、v3→v4（搜索排序派生时间与索引）、v4→v5（独立文件私有定位数据）、v5→v6（目录身份锚点与定位候选）、v6→v7（历史观察来源与修复审计）及v7→v8（补齐旧范围退休资格列）；新库也走同一迁移链。文档不再把未建的 file_resources/web_resources/identity_bindings 表描述成现存表。

029实际原生读取失败暴露此前schema7存在四列/五列两种范围表。v7→v8以单事务补齐缺列：缺少退休时资格证据的旧行置0，当前同identity读取仍服从当前权限，新capture默认1、由retire冻结实际资格；五列旧库保留已有0/1。迁移不改UUID、关系、收藏或观察来源，其他损坏结构不普遍修补。恢复源备份不变，失败回滚列/数据/版本；见[决定0027](docs/decisions/0027-schema8-history-scope-migration.md)。

```text
settings(key PK, value)                           permission_version / revision
scan_roots(id PK, path UNIQUE, active, paused, generation,
           scan_state, scanned, failures)
scan_exclusions(root_id FK, relative_path, PK(root_id, relative_path))
resources(id PK, kind CHECK file|web, name, location, identity,
          modified_at, size, status, favorite, created_at)
  UNIQUE identity WHERE identity IS NOT NULL
file_roots(root_id FK, resource_id FK, generation, PK(root_id, resource_id))
file_grants(resource_id PK/FK, active)             v2
file_locators(resource_id PK/FK ON DELETE CASCADE, identity, bookmark BLOB) v5，私有
root_locators(root_id PK/FK ON DELETE CASCADE, identity, bookmark nullable BLOB, valid CHECK 0/1) v6，私有
root_aliases(root_id FK ON DELETE CASCADE, path, PK(root_id,path)) v6，仅候选历史前缀
workspaces(id PK, name, created_at, sort_order)    sort_order 在 v3 增加
workspace_resources(workspace_id FK ON DELETE CASCADE,
                    resource_id FK, added_at, PK(workspace_id, resource_id))
activity_origins(activity_id PK/FK CASCADE, observed_resource_id nullable, identity nullable) v7，私有
activity_root_scopes(activity_id FK CASCADE, root_id FK CASCADE, root_identity nullable,
                     relative_path, retired_location_valid CHECK 0/1, PK(activity_id,root_id)) v7表，v8补齐旧四列格式，私有
resource_relinks(event_id PK/FK CASCADE, source_resource_id, merged_resource_id nullable,
                 previous_identity nullable, selected_identity) v7，私有
activities(id PK, kind, name, resource_id FK, at,
           source DEFAULT app, file_modified_at)  后两字段在 v2 增加
operations(id PK, payload, result)                 持久业务幂等回执
search_documents(rowid PK, entity_id UNIQUE, kind, name, location, modified_at nullable)  v4派生时间
search_tri / search_short                         FTS5 派生投影
```

- `resources.id` 是稳定 UUID，file 的 location/identity 与 web 的 URL 均是可变属性；工作区引用永远不绑定路径或文件名。一个文件可出现在多个工作区；另存为/副本默认独立资源。
- 当前 file/web 共表、收藏在 resources.favorite、物理身份在 resources.identity；逻辑上的 ResourceService/FileLocator/WorkspaceService 是职责边界，不要求拆成更多表或进程。022已在file_locators持久化macOS独立文件的普通Bookmark及其物理身份绑定；024增加root锚点、valid和候选aliases；Document ID和security-scoped访问凭据尚未落地。
- `file_grants` 的权限绑定资源及其已复验的身份，独立于目录覆盖；最后一份有效授权撤销后全局入口隐藏，工作区匿名占位。恢复撤销目录与单文件历史授权。当前没有保存 granted_at/原生 credential blob，普通Bookmark定位已在022做单文件增量，不代表OS sandbox权限持久化已完成。
- 活动 `at` 是应用行为/观察时间，source 为 app/filesystem；`file_modified_at` 仅是观察到的文件 mtime。025起新活动保留观察名称和来源，历史显示按观察identity及有效历史范围授权；旧空名只有当前资源也获授权时回退，未知identity隐藏。活动不保存正文或旧版本。删除网页会明确删除该网页关系/活动；删除工作区保留必要管理历史，不再提供进入失效工作区的目标。
- 业务记录与搜索投影同事务更新。NFC + Unicode lowercase 为当前匹配规范，不宣称完整 casefold 或重音忽略；搜索使用 FTS5 trigram、编码 unigram/bigram 与名称辅助索引，不读取本地文件或保存网页正文。
- 外键、关系唯一约束、身份唯一索引及小事务保护归一。当前 Catalog 和原生导入仍分别协调写入，共用相同 UUID/identity 约束，必须以并发测试证明不会重复、反向覆盖或产生重复观察事件。
- migration 以事务升级并拒绝未来 schema。开发过程中已经产生旧库与备份，必须测试其到 v8 的升级/恢复，不再以“未发布”跳过真实旧开发数据。浏览器原型 localStorage 不自动导入正式库。
- 原生 File ID 或 inode 可能复用，只命中键不构成永远连续的证明。当前保守物理身份与安全替换限制见第4节；冲突不静默按路径合并。

032原生确认由Tauri注入的调用WebviewWindow归属：恢复、重新指向、合并、拖入文件授权的消息builder显式绑定parent，macOS走窗口sheet。避免无parent时依赖进入独立CFUserNotification提醒路径；前端invoke参数、Rust授权与持久化流程不变。实际恢复警示可见、取消确认/取消选择器均释放busy且不改授权/关系；完整恢复覆盖仍未执行，见[决定0029](docs/decisions/0029-native-confirmation-window-parent.md)。

### 3.1 拖放与引用转移

- 系统适配器从 Tauri 原生 Drop 事件取得 Explorer/Finder 路径，仅在 Rust `DropInbox` 保存，创建随机 UUID token；当前单一待处理批次有效期为300秒，新批次替换旧批次。先立即发送files-drop-started，前端按token在松手时固定目标UUID；后台捕获物理身份后发送ready，再消费该目标，迟到旧token不重选当前页面。前端只收到 token/count/物理坐标，按 devicePixelRatio 转CSS坐标识别目标工作区。当前锁定Wry的macOS坐标实际为AppKit逻辑点，040在原生边界乘当前屏幕倍率后再发送；其他平台已是物理像素，不重复缩放，见[决定0036](docs/decisions/0036-native-drop-coordinate-units.md)。修前已捕获真实错位，修后Retina、Windows缩放和多屏目标矩阵仍须真机验收；依赖变化时重新核对坐标约定。
- 首次 import_drop 不扩大权限，逐项导入已有授权项；未授权项留在同一 token 中，用户明确同意原生“仅授权所选文件”对话框后再补导入。取消时成功项保留，未授权项无关系。pick_files 的明确文件选择只授予所选文件；添加父目录另走 pick_root。token 本身不构成权限，提交仍重新规范化路径、复验身份、目标工作区与当前授权。
- 内部拖动使用 Pointer Events 与7px阈值，传资源UUID集合，不使用系统文件路径；默认只加入目标，释放后抑制意外点击，取消清理目标状态。与Tauri原生拖放分开，避免Windows原生handler和HTML5 draggable冲突。
- 030在资料pointerdown通过复选框/行操作guard后阻止默认手势，保持UUID集合、目标命中与mutation路径。032纠正了最初缩略图坐标误用；以原029/当前030同纪要、同原图坐标对照确认：原实现松手残留浮层，030成功关联并清理。单项/多项/重复/非目标松手有原生证据，见[决定0028](docs/decisions/0028-native-resource-pointer-gesture.md)。
- `WorkspaceService` 用 `(workspace_id, resource_id)` 唯一约束保证重复拖入幂等。对已授权的一项，验证和提交发生在同一个业务操作内。已完成项和待授权项分批提交，并向界面保留逐项结果。
- 目标契约：执行明确的「移到工作区」时，每个资源在一个事务中验证来源引用、确保目标引用存在、删除来源引用并写活动事件。当前 UI 通过逐项正式 mutation 实现局部成功；核心多项命令与类型化批量回执的差距见2.2.1。重复请求不会删除其他引用，失败时回滚该项，不影响已处理的其他项。来源与目标相同直接返回无变化。

### 3.2 并发发现与资源归一

- 目录扫描、父子目录重叠、监听与拖入统一调用 ResourceService 的查找或创建操作；FileLocator 提供可比较身份和验证结果，Catalog 提供发现来源，不由各调用者分别生成持久资源。
- 在有效授权下解析身份，在小事务中查绑定、创建或复用 UUID、登记当前目录归属及索引变更；新增单文件授权仅由 RootPermissionService 在明确授权流程中写入。依赖唯一约束处理竞争，冲突时重新读取或明确失败，不能另建第二个 UUID 绕过。
- 名称和路径不作为长期唯一主键，复制和另存为默认新资源。缺少可靠身份的场景标为有限支持/待确认，不按同名自动并入已有用户资源，也不能每次扫描无限产生重复记录；具体受限归一规则须由 S01/S02 验证后记录。
- 归一验收包含父子目录扫描与拖入并行、重复事件、改名后重新发现、删除后同名重建、原生 ID 复用疑点，以及物理/文档标识不一致；不要求首版支持所有硬链接或特殊文件系统。

### 3.3 历史与生命周期

- 工作区删除前，在事件载荷保留必要的工作区名称与删除事实；外键置空后不再生成可进入的工作区链接。载荷采用按事件类型声明的字段，禁止塞入任意路径/资源详情快照绕过撤权过滤。
- 重新指向和合并是当前发生的独立应用事件，记录原始 UUID 与目标 UUID 等必要来源；旧事件可以解析到当前规范资源，但不得被描述成过去已操作新文件。更新活动引用与合并关系在同事务完成。
- 025以schema v7私有activity_origins/activity_root_scopes/resource_relinks保存观察对象与修复来源。历史名称按观察identity或原有效目录锚点/相对范围授权，不借新规范对象的权限复活；当前对象撤权时其DTO仍匿名禁用。v6旧事件只建立迁移时权限基准、观察UUID为NULL，不声称重建原始来源；当前格式缺来源不作读取兜底。迁移和恢复失败不得覆盖当前数据，详见[决定0023](docs/decisions/0023-history-origin-and-relink-review.md)。
- 026将同一谓词改为显式CASE：观察identity与当前资源相同，非空identity唯一约束保证只检查一次当前权限；异identity使用标量查询区分已有对象的0/1权限与无对象NULL，只有NULL可查询历史scope。旧空名的当前规范资源授权门槛仍约束整个异identity分支；overview、明细、日历与诊断共用，不建立第二份权限缓存，见[决定0024](docs/decisions/0024-current-identity-history-permission.md)。
- 用户保存的资源、工作区关系和收藏不因失联自动删除。未被保存的扫描条目与观察事件的清理策略在发布前明确；确定前不擅自清除用户数据。工作区与文件的历史读取仍按当前授权过滤。
- 文件变化事件按范围/批次合并并限频，避免每个底层通知都成为一条用户动态。日志、队列、WAL、观察历史与备份均纳入长期体积测试；保留/轮换策略和资源上限由 S05 报告明确，不能以无限积压维持表面成功。

## 4. OS 文件身份与定位策略

### Windows

- 针对支持的本地文件系统尝试使用 `FILE_ID_INFO`（`VolumeSerialNumber + FileId`）。
- 支持时验证 `OpenFileById` + `GetFinalPathNameByHandle` 能否找到同卷改名/移动后的路径。
- **不能**假设跨卷移动、所有磁盘格式或 Word/WPS 安全保存后 ID 不变。
- 区分 File ID 与可选的 NTFS Object ID。Microsoft `ReplaceFile` 保留原 Object ID，但结果采用替换文件的 File ID；实际编辑器是否采用相应保存流程由 S01 验证。
- 默认仅用 `FSCTL_GET_OBJECT_ID` 查询已有Object ID，不存在不视为异常。2026-11-21用户批准的例外：有效内置NTFS目录显式启用追踪后，对收藏/加入工作区的文件按需调用 `FSCTL_CREATE_OR_GET_OBJECT_ID`；读取与创建分离，逐文件记录成功/失败及授权代次。不得默认给所有扫描文件创建，不写ADS、标签或隐藏文件；停止追踪/撤权不删除已有标记。
- 物理 File ID 变化后先评估已验证的文档延续证据，不仅凭同路径/同名换绑。证据不足或冲突时转入 `identity_unconfirmed` 与人工修复流程；Distributed Link Tracking 的启发式结果不能自动充当确定身份。

正式实现遵循[连续性分层方案](docs/WINDOWS_DOCUMENT_CONTINUITY_ARCHITECTURE.md)，并以本段修正早期实验建议：持久锚点足够且通过授权/版本/冲突复核时，缺在线事件不单独否决退出期间恢复。事件是候选发现机制，不以进程名或时间窗替代锚点。Resource UUID、物理identity、文档锚点保持独立；当前访问、历史展示与逻辑连续性分别验证。连续性提交统一更新绑定、旧单文件授权失效、历史依据、搜索投影和版本，失败整体回滚；打开前继续物理复验。历史观察identity不可改写，recent必须在全部查询/排序入口统一解析连续性链。目标identity已有资源时拒绝静默合并；先强制验证WPS临时源提前入库时序，再决定是否需要显式的纯扫描记录规则。目录同意与逐文件锚点结果、迁移/备份恢复/多次保存重启均为正式验收项，不能只更新Windows技术说明而忽略PRD规则。

官方：https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_id_info
官方：[ReplaceFile](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilew)、[查询 Object ID](https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ni-winioctl-fsctl_get_object_id)

### macOS

022已实现的范围是活跃独立file grant：普通Bookmark私有持久化，原路径失效后只在物理身份与授权复核通过时恢复一次。Foundation在gate/SQLite写锁外，原ingest事务复核快照后更新；旧grant可在当前位置核验后补建，目录扫描不全量捕获。relink/merge同步清理旧绑定，恢复不复活授权；有效单项定位错误先完成无关root扫描后明确返回。详见[决定0020](docs/decisions/0020-single-file-native-locator.md)。024接入整个root外部移位与仅目录授权引用的同物理身份定位：启动仅metadata核验、锁外Foundation、CAS提交；新授权有锚点，legacy需明确重选，暂停只核已有映射。有效scope统一进入查询/扫描/导入/relink，后台30–300秒退避核对空源与暂停根，详见[决定0022](docs/decisions/0022-root-native-locator.md)。普通Bookmark在旧路径被新对象占用时可能给出替代对象，必须拒绝并明确重选原目录；下列Document ID等更广能力仍为目标策略。

- 对进入手动工作区、收藏等重要关联的文件持久化 `NSURL` bookmark data，按需恢复；普通索引项可以优先保留较轻量元数据。
- 候选加入 `URLResourceKey.documentIdentifierKey` 作为同卷文档延续证据。Apple 明确其可跨重启、同卷移动并延续 safe-save，复制时不转移；并非所有卷支持。S02 同时验证 Bookmark 实际恢复与 Document ID，不先假定 Bookmark 在保存后必然失败。
- Document ID 返回 nil 时按不可用处理，不以 nil==nil 作为身份保持证据。本机首轮只读探测尚未取得该值，不能默认 APFS 上每个文件都可用，详见实验结果；Bookmark 的保存/替换恢复仍须独立验证。
- Bookmark 负责位置解析和适用时的访问凭据；Document ID 不能替代定位和授权。`fileResourceIdentifierKey` 不保证跨重启持久，不直接当作长期身份键。
- macOS 打包是否沙盒化会影响 security-scoped bookmark 和持久目录权限；需要独立测试不同签名/分发方式。
- Bookmark 解析成功但 stale 时，在身份及授权复核后刷新持久定位数据；解析失败、权限撤销或跨卷等不能确认的情况保留关联并反馈，不能把 stale 一律当成失联。
- 本机原生样本中，Bookmark 在原子保存/替换后可解析，也会解析到删除后同名新建的对象。因此“解析成功”或 stale 状态只作定位证据，不能单独证明文档连续性；无额外身份依据时仍不能自动接回旧关系，详见 [阶段实验结果](spikes/RESULTS.md)。

官方：https://developer.apple.com/library/archive/documentation/FileManagement/Conceptual/FileSystemProgrammingGuide/AccessingFilesandDirectories/AccessingFilesandDirectories.html
官方：[documentIdentifierKey](https://developer.apple.com/documentation/foundation/urlresourcekey/documentidentifierkey)、[fileResourceIdentifierKey](https://developer.apple.com/documentation/foundation/urlresourcekey/fileresourceidentifierkey)

### 4.1 失联与重新指向

- 原生适配器解析位置，Rust 根据平台已验证的物理/文档身份证据判断连续性，重新校验当前授权，再调用默认应用或系统文件管理器。仅有定位候选不构成可打开授权；原生标识也不是业务内容相同的证明。
- 物理标识变化时，先尝试启用的文档延续策略。身份明确且授权仍有效时保留资源 UUID 与全部关系，更新绑定和缓存定位；不要求用户仅因安全保存手动修复。目标 OS/文件系统/编辑器支持矩阵和判定规则由 S01/S02 固定。
- 仍无法确认、证据冲突或不同对象占用旧路径时，保持原资源 UUID 与关系，标为待重新指向。只有文件名、缓存路径、大小或修改时间匹配不能自动恢复。跨卷、云占位符与关闭应用期间保存分别记录结果。
- 手动重新指向先显示受影响的工作区，再由用户明确选择候选文件。提交时复核候选身份和权限，保留原资源 UUID 并更新定位信息。候选已作为另一条资源存在时，先提示合并影响。经用户确认后，同一事务内归并工作区关联、收藏及活动引用，按唯一约束去重，更新当前文件身份和有效授权，再移除重复记录。原件身份上的单文件授权不能沿用到新对象。
- 系统接受打开请求后才记录应用内打开；请求被拒绝或提交结果不明不记录为已打开。安全检查与按路径交给 OS 之间的竞争窗口须在 S04 评估，不能将一次路径规范化宣称为完全消除 TOCTOU。

### 4.2 可关闭的网页标题获取

`src-tauri/src/web_metadata.rs` 独立于离线搜索与 Catalog；用户在新网页弹窗默认开启自动标题，可立即关闭。URL 有效后600ms去抖；初始名称为域名，手动修改、URL变更或弹窗卸载后旧响应不得覆盖输入。获取失败、无标题或关闭增强都可以手工命名并保存；不使用最终重定向URL擅自替换用户保存的原URL。

- 只接收最多8192字节、不含用户名/密码的 http(s) URL；移除请求 fragment。只获取HTML，解析 `<title>` 纯文本，不执行JS、不读取浏览器cookie/登录态，不加载预览图或favicon。
- 当前解析经固定 `https://cloudflare-dns.com/dns-query`、TLS主机名与1.1.1.1:443，查询A/AAAA；目标请求禁用系统代理和自动重定向，每跳重验DNS全部地址并固定连接地址。此设计会向Cloudflare暴露请求主机名，向目标网站暴露请求IP及URL路径/查询；弹窗披露该行为，关闭自动标题后不启动该请求，不修改系统DNS。
- 仅允许公网地址，保守拒绝私网/loopback/link-local/保留网段/IPv4映射等；单标签与local/internal等名称不自动抓取，可手填保存。企业代理/内网站点因此可能不能自动取标题，不能静默降低校验改为任意网络代理。
- 总预算5秒、最多3次跳转、HTML读取256KiB、DNS响应16KiB；标题最多200字符，去控制字符并压缩空白，仅支持可验证UTF-8/ASCII。超时、超量、错误MIME/编码/状态码均返回可见失败，不阻断本地保存。
- 关闭开关/弹窗和输入变更当前通过序号丢弃响应；已发出的后端网络请求可能在总预算内继续，不能承诺撤销后网络立即停止。对应公网解析与真实HTTP行为、重定向/重绑防护、手改保护与离线使用需独立测试。

## 5. 索引、监听与重新核对

### 初次扫描

1. 只对用户明确允许的目录进行目录枚举，不默认全盘。
2. 读取文件基本元数据；异步分批提交 SQLite，暴露进度、取消与错误。
3. 默认排除典型大型开发/系统缓存目录（如 `.git`、`node_modules`、缓存）并允许用户修改配置。
4. 增量显示数据，不等待全部完成才允许查询和工作区使用。
5. 不打开读取文档正文，不预加载大量图片/视频，不触发云占位文件下载。
6. 007在搜索投影自身比较规范化kind/name/location，两套FTS行均存在且内容未变时不重写；投影/FTS行缺失仍修复，显式重建仍强制重建。FTS行存在但内容错误依赖显式重建。021投影显式接收资源mtime，resources为事实源；Catalog/导入/重新指向/网页保存使用同一值在同事务更新。仅mtime变化时只更新一行搜索投影及其排序索引，不重写FTS；不变mtime保持零投影写入；工作区为NULL，资源0/负时间保留。v3→v4迁移一次回填，不在每次打开全表更新。非唯一resources(location)派生索引加速同路径检查，旧v3/恢复暂存库打开时幂等创建，首建成本另验。

### 变化跟踪

- Windows V1：原生 `ReadDirectoryChangesW`（或 Rust 跨平台抽象底层已使用的等效原生机制）；必要时评估 USN journal，但首版不依赖管理员权限/第三方 Everything 进程。
- macOS V1：`FSEvents`，注意事件合并和 `MustScanSubDirs` 等需要补扫。
- 事件批处理、去抖合并后更新本地 catalog；数据库失败可安全重试。
- 应用关闭无需常驻监听；重新打开核对已授权位置。当前仍完整枚举元数据，007只复用不变搜索投影，不称文件系统差量扫描。已测10k与100k实际合成小文件的扫描并发查询成本；009批次以独立mutex串行写入，持共享恢复屏障并用Immediate事务，查询使用单读快照，见[决定0008](docs/decisions/0008-scan-read-concurrency-and-restore-epoch.md)。空闲CPU/RSS、真实办公文件/云目录与事件洪峰继续验收。
- 单靠监听不等于“正确”；事件遗漏时通过受控差异扫描/身份解析恢复。
- Catalog和独立单文件复验仅在首次资源归一时写file_discovered；原identity的mtime或size变化时写file_modified；不变重扫/重叠根不重复产生事件。at为观察时间，file_modified_at保存当时mtime；只记录观察事实，不把文件元数据伪造成精确用户编辑审计。独立授权文件纳入监听与启动/重新扫描复验；不为此扫描其未授权父目录。
- 通知洪峰应限流/合并；动态事件写入不得阻塞文件索引和搜索。应用未运行期间的完整修改序列不能被 V1 承诺恢复。

### 5.1 扫描完成与并发收敛

- Catalog 为每个根分配扫描代次，记录范围、授权版本与完成状态；`last_scan_at` 只表示时间，不能替代完整完成证明。目录枚举、定位和数据库提交分批进行，UI 可取消。
- 只有范围仍有效且完整枚举成功，才能将该范围内本代未发现的旧项标记为疑似缺失并交定位复核。取消、访问拒绝、部分失败或卷离线不能据“没看到”确认删除，也不删除用户关系。
- 扫描与监听共用提交协调；扫描读取前记录业务修订，提交时检查是否已有更晚观察或授权变化。过期批次不覆盖新路径/身份，冲突范围进入有限补扫。文件 mtime 不是并发排序或授权依据。007提交边界重读当前metadata并复核身份及canonical path，以当前mtime/size比较；复验失败累计进扫描failures，收尾不将其判为完整无失败枚举。不宣称这些独立系统调用消除了检查后的文件变动窗口。
- 监听溢出、游标断裂、未知顺序或队列达到上限时标记对应范围需核对，合并范围后补扫，不静默丢弃并报告“已同步”。应用关闭期间不保证获得逐条事件，重启依据未完成状态、可用游标与范围核对恢复。
- 撤权先阻止旧任务继续提交可见数据，再停止对应监听；完成回调必须复核当前授权版本。数据库内授权与提交判定保持一致，文件系统实际状态仍在打开时重新验证。
- 队列、并发数、事务批次、可重试错误及重试次数/耗时上限由 S03/S05 实测确定并进入配置；持续权限拒绝、磁盘满等不可无限重试。状态与失败原因对用户可见，不自动放宽扫描范围。

官方：https://learn.microsoft.com/en-us/windows/win32/fileio/obtaining-directory-change-notifications
官方：https://developer.apple.com/library/archive/documentation/Darwin/Conceptual/FSEvents_ProgGuide/UsingtheFSEventsFramework/UsingtheFSEventsFramework.html

### 5.2 日历文件时间线契约

- `TimelineRequest` 携带可选from/to、month_from/month_to、day_boundaries、filter、basis、workspace_id、offset；时间为Unix毫秒，拒绝非有限/倒置区间。day_boundaries 必填，2–32个有限、严格递增的边界，首尾须与月份一致、范围不超过32天；显示端逐本地午夜构造。filter为all/modified/discovered/opened/system，all默认只含文件修改、首次发现和应用内打开，管理事件通过system次级筛选。
- basis默认file_modified；仅file_modified事件且有有效mtime时以file_modified_at为effective_at；首次发现永远按at，不把旧文件放成过去的创建事件。observed模式全部使用at；UI同时展示可解释的另一时间及来源。
- React按本地时区构造选日、下日午夜、月份和逐日本地午夜的半开区间，不能固定加24小时。Rust在同一SQLite读事务内用同一授权/工作区/类型条件查询分页明细与整月按日计数，不按UTC截断。每日本地边界走独立索引范围计数，避免逐条传输精确时间或执行31路CASE；file_modified有效时间增加派生表达式索引，旧v3和恢复暂存库打开时幂等创建，不改变领域schema版本。
- 每页100事件，(effective_at DESC,id DESC)稳定次序；CalendarPoint返回当天本地边界/数量，仅返回有活动的日期，最多31点。019追加页须回传绑定完整条件/session/permission_version/revision的末项游标，SQL按有效时间/ID双键继续且不再OFFSET；变更明确stale_cursor，UI仅续页一次重读首屏。见[决定0017](docs/decisions/0017-timeline-seek-cursor.md)。014已完成100k混合实体/976页时间线的独立正确性核对但性能超标。017增加反向关系覆盖索引及日期范围条件；018按当前读取事务的权限/revision及完整日历条件复用最多4份月历计数，只缓存计数，正文每次读取。首次未命中成本、深offset、原生渲染与长期事件存储继续单列验收；见[决定0016](docs/decisions/0016-versioned-calendar-cache.md)。
- 快速返回文件来自当前时间范围已加载事件，不等同于全局最近打开；相关工作区来自当前资源关系，不声称历史发生当时的工作区快照。撤权后名称、快速返回和日历计数一起过滤。

## 6. **最高优先级：统一极速搜索服务**

### 6.1 搜索语义与排序

- 跨所有主页面在**右侧主内容顶部**使用同一处固定全局搜索框，左侧主导航保留；快捷键 `⌘K` / `Ctrl+K`；展开大型分类弹层，按「工作区 / 文件 / 网页」分组，支持方向键、Enter 打开、Esc 关闭。
- 普通搜索默认范围为 `all`，可显式选择 `current_workspace`。面板始终显示范围。工作区「添加文件」使用全局范围加文件类型过滤，目标工作区只作为加入参数。完整结果页继承当次查询的范围和来源页面，支持按类型、日期和路径筛选。
- 文件索引字段：`file_name`、`cached_path`、扩展名、mtime；网页：`display_name`、`url`；工作区：`display_name`。首版**不索引文件正文或网页正文**。
- 排序按精确名称、名称前缀、名称子串、路径或 URL 命中的顺序。同档次权重用实际样本调优。分组用于展示，由全局相关性决定默认高亮，Enter 打开当前高亮项。同名且同档次时显示可区分的路径，等待用户明确选择。
- 中文 1–2 字文件名搜索按子串匹配。`张` 和 `张三` 均须命中 `合作合同_张三.docx`。失联且仍有授权的文件可显示，状态标明当前无法打开。

### 6.2 热查询实现约束

- 每次输入只查询**SQLite 已建立的本地索引**，不得遍历磁盘、等待网络、打开 Office/PDF 或触发云占位文件下载。
- 当前采用FTS5 trigram和预生成编码unigram/bigram；先估计最多2049个候选，低命中走FTS，高命中走按名称辅助索引的精确/前缀/子串/路径四档查询。每档权限/类型/工作区条件均在LIMIT前，不按预取Top-N后丢弃。此优化已跑Rust合成磁盘基准，但不能由该分布推断所有授权/查询组合达标。
- 当前UI关键词与路径输入80ms尾部防抖、请求序号丢弃旧响应，按工作区/文件/网页分组；快速搜索最多30项，all且匹配>30时每类预留至多10项，稀少类别空位按全局相关性补齐。类型候选使用派生(kind,name,entity_id)索引，所有授权/范围条件在limit前。完整结果仍保持纯全局排序。search_page每页100项，支持类型过滤及相关性/名称/修改时间排序。第一页不带游标，有后续页才返回next_cursor；后续页必须携带游标及对应next_offset。后端核对协议版本4、Store session/权限/revision和全部规范化查询条件；过期或缺失返回stale_cursor，UI仅对后续页清掉旧列表和选择并重读首页，首页报错不循环重试。016名称排序按规范化name/ID末项续读，next_offset仅保留逻辑计数；020相关性携带rank/name/id末项位置，按精确/前缀/子串/仅路径四个互斥档次续读；高命中复用名称索引，低命中保留FTS，所有筛选在LIMIT前。021修改时间增加独立nullable时间/name/id位置，同时间余项、较早时间、NULL工作区尾段分别续读；高命中使用时间或kind+时间索引，低命中FTS复合位置，所有排序OFFSET均为0。三种位置槽互斥，旧版本、缺失位置或错误位置槽明确stale_cursor。修改时间决定与证据见[决定0019](docs/decisions/0019-modified-projection-and-seek.md)。相关性决定与性能见[决定0018](docs/decisions/0018-relevance-rank-seek.md)。见[决定0014](docs/decisions/0014-complete-search-name-seek.md)；深分页及原生性能按排序分别验收。Core每请求独立SQLite连接/progress_handler执行取消，native固定quick/page槽和查询会话防止旧请求抢占；App清理显式取消，模块序号不因组件重挂归零。013修复分组后的默认高亮、返回来源上下文与重置加载集合时的旧选择，见[决定0011](docs/decisions/0011-selected-workspace-and-search-context.md)。实现与边界另见[决定0003](docs/decisions/0003-search-cancellation-and-groups.md)。
- 完整搜索 `SearchPageRequest` 新增可省略的 modified_from/modified_to/path_prefix，不改变快速 Top30 算法。时间为有限的 Unix 毫秒，使用 `[from,to)`，拒绝倒置/空区间与超出 Date 范围的值；工作区无 mtime，启用任一时间边界时排除，网页使用当前 resources.modified_at。路径或网址前缀按现有 NFC + lowercase 投影做字面前缀匹配，`%`/`_` 不作通配；不展开 `~`、不解析相对路径、不读磁盘，空串无筛选。路径前缀不是授权规则；授权、工作区、类型、时间与路径条件均在 LIMIT/OFFSET 前应用。前缀不擅自去除真实路径空格，NUL 或超过8192字节返回 invalid_input。专用核心回归见 tests/search_filters.rs，UI 输入/渲染验收另记。
- 授权、资源类型及工作区范围过滤进入候选选择与分页过程，不先取 Top-N 再丢弃未授权项。返回前复验授权版本；权限外条目不能泄露，也不能挤掉合法匹配。
- 用户输入作为字面量查询处理，SQL 参数化之外仍须处理 FTS 表达式与 LIKE/GLOB 元字符；不默认开放高级查询语法。S03 固定大小写、Unicode/重音与符号的匹配规则，保留原始显示文本；检索规范化不能用来合并物理文件。
- 同档次排序增加稳定实体键作为决胜条件。分页游标包含规范化查询条件与版本；投影/范围变化导致游标不适用时返回stale_cursor并刷新，不能混用两次查询的游标或宣称跨更新仍是完整快照。游标是客户端回传的版本化描述，不是签名凭证或授权，SQL仍逐页执行当前授权。快速和完整查询各自在同一SQLite读事务内组装候选、资源和版本，保留运行中取消；不持有跨请求事务。实现见[决定0005](docs/decisions/0005-versioned-search-pagination.md)。
- 数据库使用预编译参数、事务、合理复合索引；后台索引写入与热搜索读取隔离，避免写锁/大事务卡住交互。SQLite WAL 与 checkpoint 策略须实测，不能假设默认配置最优。
- FTS 失效应显示可恢复状态，支持重建，不影响基础资源和关联；新增资源、网页标题修改、移动文件、更改工作区名后索引最终一致且有延迟上界测试。
- 分别测量取消后的后台查询是否停止消耗资源，不能只在 UI 丢弃结果就认定已完成后端取消；连接与取消粒度不能误中其他正在执行的请求。

### 6.3 可测量的候选性能预算（上线门槛待 Spike 批准）

| 指标 | 候选目标 | 条件 |
|---|---|---|
| 搜索框点击/快捷键到可输入 | P95 ≤ 50ms | 主窗口已运行 |
| 已建索引输入到首批结果**可见** | P95 ≤ 150ms、P99 ≤ 300ms | 10 万混合资源、温热缓存、真实渲染、至少 500 次采样 |
| SQLite 搜索核心查询 | P95 ≤ 60ms | 同数据、记录实际 SQL+查询计划、热查询 |
| 搜索过程中 UI 主线程 | 无肉眼卡顿，长任务不在主线程 | 初次索引与文件系统事件洪峰并行 |
| 大结果集 | 下拉先 Top-N（建议每组 5–10 条），完整页分页/虚拟列表 | 不一次性向 React 传回全部结果 |

以上是候选交付门槛。007记录Rust release磁盘合成100k热查询P95为9.393ms、并发收藏写入9.524ms，500k压力36.709ms，见[验收记录](docs/ACCEPTANCE.md)；这不覆盖原生输入至结果可见、完整结果页、扫描写入或所有筛选负载。报告必须提供 Windows/macOS、CPU/内存、磁盘、索引数量、查询集、冷热、P50/P95/P99、误差和失败实例。如果 Spike 发现指标不可行，先报告取舍并请产品负责人批准变更。

S03 还须记录实际打包的 SQLite 版本/编译能力、索引体积、首次构建与增量更新成本、冷启动到可搜索时间、高命中单字查询、长路径和取消压力。500 次采样是起点，报告尾部样本量与波动，不把少量样本的 P99 当稳定结论。异步投影的新鲜度上界与积压反馈须在方案定稿时固定。

### 6.4 搜索正确性与回归集

覆盖中文 1/2/3+ 字、中文与英文混合、大小写、重音、符号/空格、文件扩展名、路径含空格、`张三`/`2026招聘`、相同文件名不同目录、URL 中参数、工作区重名、范围切换、取消/删字竞态、未授权文件、失联和离线资源、扫描中新增、重建索引。另建立真实用户常用词的 Top-5 命中率和误命中记录，不允许只测随机英文字符串。

端到端回归需覆盖中文输入法仍在组字时按回车、完整文件名与前缀匹配工作区同屏、工作区范围切换后使用「添加文件」，以及已有下拉结果时并发撤销授权。真实渲染采样需覆盖索引写入并行。

官方：https://www.sqlite.org/fts5.html

## 7. 权限边界与数据安全

- **Rust 执行层**对每次索引、读取、打开、定位操作检查授予的根目录或单文件权限，不信任前端路径。
- 对符号链接、规范化路径、不同盘符大小写、父子重叠目录、移出范围文件的处理需有测试。
- 不把 Tauri capabilities 误当成对所有自定义 Rust 命令的自动授权限制；主动实现命令级校验并显式限制可调用窗口/WebView。普通页面不能直接调用绕过业务服务的通用 fs/shell/opener 命令。
- 不提供原件删除、复制、移动、重命名写命令；测试任何原件行为只对临时 fixture 操作。
- 当前有效授权取目录覆盖与单文件授权的并集。暂停扫描保留已有授权，排除子目录按对应授权范围生效。撤销目录时停止该目录扫描与监听，重叠目录和单文件授权继续独立判定。
- `file_roots` 是最近确认的归属投影，不是历史发现即永久授权。热搜索不为每项访问磁盘，按当前授权规则与最近确认归属过滤；外部变化在被观察前可能有元数据延迟，不能承诺关闭应用期间实时撤除。打开/定位时重新解析实际目标、路径覆盖和排除规则，发现移出则更新归属并拒绝未授权访问。
- 原生文档身份确认连续时，只有平台访问凭据仍有效且授权规则覆盖该目标，才能维持访问。重新指向另一身份时不沿用旧单文件授权；目录覆盖或目标对象已有授权可继续有效，但仍需重新核验。原生picker可通过relink_selected仅授明确选中的新文件，捕获身份在后续合并确认后仍复用，提交前再核对；grant合并取active并集，不因保留UUID曾被撤权而覆盖目标对象现有授权。
- 最后一份授权撤销时，对搜索、最近使用、收藏入口和动态按当前授权过滤，返回前作废旧查询，清除界面已显示的名称和路径。工作区读取只返回匿名占位和重授权入口。可用授权版本号串联写入、读请求和前端事件。索引删除或重建可随后完成。
- 日志默认只记录必要的技术错误，避免写入用户敏感的真实路径和文件名。备份范围和恢复契约见 7.1。
- 不在应用首次启动时要求系统全部文件访问权限；不要求用户关闭系统安全保护。
- 文件名、工作区名、网页标题和 URL 均按不可信文本渲染；限制 WebView 导航/弹窗与远端内容加载，不因收藏图标隐式联网。URL 与文件打开均走校验后的业务命令。程序、脚本、快捷方式和应用包的直接启动策略待 PRD 第 10 节确认，确认前不实现该路径。
- 授权撤销不是数据擦除：数据库、WAL 和备份可能含历史元数据。导出诊断需说明范围并脱敏，不能把备份/日志原文直接发送模型或外部服务；产品仍无默认遥测。用户主动移除应用数据时明确当前库、缓存和可管理备份的范围，不影响源文件或承诺擦除用户另存副本。

官方：https://v2.tauri.app/security/capabilities/

### 7.1 本地备份、恢复和升级

- 备份包含工作区、稳定资源 UUID、关联、URL、收藏及必要定位元数据，不包含源文件。应用数据和备份保存在用户指定或系统应用数据位置，搜索投影恢复后可重建。备份可能含本地路径与敏感 URL；同盘备份不能作为整盘丢失的恢复保证。
- 通过明确的用户入口发起备份/恢复，升级前留一份可校验快照。当前运行中一致快照使用 SQLite Online Backup API，不能直接拷贝一个活动中的主库文件而忽略 WAL。
- 恢复先写临时库并核对版本、完整性、外键和业务计数；切换前停止写入并关闭对应连接，按已验证的原子切换与 WAL 处理流程启用，失败保留当前库。启动新会话并使旧查询/事件失效。具体备份保留/轮换和磁盘空间上限由发布策略确认。
- 当前恢复采用更保守的已披露策略：暂存/校验并迁移旧备份到当前schema，恢复前Online Backup保存当前库，恢复后撤销全部scan_roots与file_grants授权、清空旧操作回执、递增版本、重建搜索，用户重新授权。当前在 Store 写锁内以恢复前当前库的 permission_version/revision 各加一覆盖暂存库版本，而非沿用备份旧版本；正常计数范围内不会因恢复而回退版本。该策略保证不扩大恢复前范围，不按UUID/旧路径恢复权限；未来若保留部分有效授权，仍须以恢复前授权为上限并复验目标身份/OS权限。
- 记录应用版本、schema 版本与备份版本；旧程序遇到不支持的 schema 必须拒绝写入。回退验收覆盖匹配的程序和数据组合，不能只恢复旧库却继续运行不兼容的新程序。
- 用含混合资源与多工作区关联的备份做完整恢复，核对 UUID、关联数量与 URL。重建搜索投影后验证仍有权限的原件可打开。模拟迁移失败及无效备份，验证能恢复到可用状态。 [SQLite Online Backup API](https://sqlite.org/backup.html)

### 7.2 故障范围与恢复职责

| 故障 | 处理与可承诺边界 |
|---|---|
| 投影延迟、逻辑不一致或索引版本升级 | SearchService 显示状态并重建，保持业务记录与关联；成功校验后切换投影 |
| FTS 结构异常 | 隔离对应搜索操作并验证能否重建；不能吞错后返回正常空结果 |
| 数据库物理损坏、I/O 错误或磁盘满 | 停止不安全写入，明确错误；需要时从已验证备份恢复，不承诺只重建 FTS 即可修复；015已测SQLite复制层FULL、忙锁/部分复制回滚，非OS磁盘满或掉电证明 |
| 迁移失败或数据版本不兼容 | 保留原库/备份，使用已验证的程序与数据组合恢复；不能跳过失败迁移继续写库 |

V1 保持业务与搜索同库，不为假设风险提前拆数据库。“可重建投影”表示派生数据可再生成，不表示同库物理损坏时业务表天然隔离。S05 必须分别注入逻辑、迁移、存储故障验证上述边界。

## 8. 必做技术 Spike（批准全量开发前）

| Spike | 系统 | 场景 | 输出 |
|---|---|---|---|
| S01 文件身份与保存 | Windows NTFS | 改名/同卷移动、应用与系统重启、跨卷、Word/WPS 连续保存/安全替换、复制/另存为/同名重建；对比 File ID 与已有 Object ID | 身份变化原始记录、正确打开与 UUID/关系核对、无需人工修复/需人工修复矩阵；不创建 Object ID |
| S02 Bookmark 与文档身份 | macOS APFS | 同 S01，比较 Bookmark 解析/stale 与 Document ID；重新授权、撤权、离线及正式签名权限差异 | 真实机器演示、每种保存路径的定位/文档标识/授权证据及恢复矩阵 |
| **S03 极速搜索（阻塞 P0）** | Win/macOS | 1 万/5 万/10 万资源，另 50 万压力集；中文短词、路径/URL/工作区混合；索引写入并行、快捷键打开 | P50/P95/P99、首屏可见时延、SQL profile、内存/CPU、错误率、回归结果 |
| S04 拖放/权限 | Win/macOS | Explorer/Finder 多选、应用内拖放、重复加入、混合授权及取消、跨区转移、撤权与重叠目录 | 逐项结果、原件未写入验证、可复现录屏、平台限制 |
| S05 资源与数据恢复 | 跨平台/SQLite | 并发归一、扫描中断/事件乱序或溢出、引用转移/合并、历史与数据增长、备份、迁移失败、FTS 不一致/损坏、磁盘满及授权复验 | UUID/关系校验、重放与故障注入、恢复到可用版本组合、资源体积与队列/重试预算 |

所有 Spike 必须在人工指定的临时目录下生成 fixture，**不得借用真实用户工作文档进行改名、移动、删除测试**。

公开发行前，分别记录标准用户账户的安装、首次启动、离线使用和升级结果。Windows 重点覆盖缺少 WebView2 的环境及下载安装包时的系统提示。2026-11-21用户确认当前先采用无系统发行签名的分发方式，付费签名/公证后置；安装指引、版本摘要与更新包验签仍需提供。macOS验证实际分发包的首次打开和拖放授权，未来启用Developer ID/公证后再复验对应权限差异。安装与地区分发验收见 [PRD](PRD.md#91-本地数据恢复与发布验收)。

S01/S02 的“保存成功”指无需人工修复即可从原 UUID 和工作区打开正确文档，不仅是 API 返回成功或条目仍存在。支持矩阵记录具体 OS、架构、文件系统、编辑器版本、目录类型和应用是否运行。2026-11-20用户明确V1仅覆盖内置磁盘上的普通本地目录，网络盘、外置盘及云同步目录适配不纳入本轮开发/验收；仍保留位置不可用及身份不明时的基本保护。未执行、失败、有限支持分别列出。若普通保存不达标，先处理策略或支持范围，再扩大开发；不把它推断成原生跟踪路线不可行。

### 8.1 实施关口

| 关口 | 工作 | 进入后续阶段的依据 |
|---|---|---|
| G0 | 按 PRD 研究首批任务；并行做 S01/S02/S03 与安装权限验证 | 用户事实与关键技术报告；未达标项有明确处置 |
| G1 | 建立 DTO/状态/迁移及统一验证入口；实现一个原生纵向闭环 | 授权、搜索、关联、打开、保存、重启取回有原生证据；关键检查可发现故意引入的回归 |
| G2 | 在已验证平台邀请内测，逐步完成 F01–F14 及其余 Spike | 真实跨日复用、修复成本和异常恢复记录；不等同完整 V1 已完成 |
| G3 | 明确许可、支持矩阵和发行方式；验收备份、安装和升级 | 对应代码版本的制品与证据，完整 V1 范围满足后再正式发行 |

这不是新增排期承诺。工程可按已确定边界继续做可逆增量；涉及首发平台、收费/许可、用户行为变更或指标放宽的待决策项按 PRD 第 10 节处理。

## 9. 非搜索性能与资源占用（候选目标）

- **搜索以第 6 节的端到端可见时延和正确性作为首要门槛**；不能只报单次 SQL 执行时间。
- 空闲 CPU 争取 < 1%，空闲 RSS 目标 < 250MB；必须分别测量 Windows 和 macOS。
- 首次扫描 UI 可交互、可取消，重复启动无需每次阻塞全量扫描。
- 每一次数据库操作不会导致源文件写入。

以上是**建议验收目标**，不是已验证性能事实。环境、样本、耗时分布和偏差须写入测试报告。

## 10. 将来可扩展但当前不开发

- smart collections 自定义日期、类型、组合规则。
- 富文本笔记、任务、提醒、激励成长及 AI Agent。V1.3.1已实现独立全局纯文本便签与静态伙伴，不能再视为范围外。
- 浏览器扩展一键收藏当前页面、实时标签/会话管理；首版只支持手动 URL 引用，任何网页都使用默认浏览器打开，不内嵌网页预览。
- 全文内容索引、OCR、embedding、可选本地/远程 AI 分类。
- 用户明确授权的微信/钉钉/飞书附件来源与人物/群关系桥接。
- 多设备同步、云服务、团队协作、可选付费扩展。
- 后台托盘驻留、NTFS USN 增量高优化、系统右键菜单集成。

## 11. Agent-Ready 扩展边界（长期架构约束；不进入 V1 实施）

### 11.1 原则：通用领域核心，垂直应用层，可替换 Agent

- **Rust Core 是权威业务执行层**：`ResourceService`、`WorkspaceService`、`SearchService`、`ActivityService`、文件定位与权限服务不依赖模型、Agent 框架或 JavaScript 运行时；继续以类型化 API、事务和应用 UUID 维持数据语义。
- **Agent 是上层调用者**：未来自然语言任务调用与 UI 相同的 Rust 业务服务，不直接修改 SQLite、搜索索引或受管文件，也不能绕过权限检查。具体 HR/行政流程由更上层的可版本化模板和策略组合，不渗入通用 Resource Schema。
- **运行时可替换**：可选 Rust 原生 Agent Loop（如 Rig/Swiftide）或 TypeScript Agent Sidecar；界面及领域数据不依赖特定模型提供商、Prompt、Skill/MCP 框架。
- **外部 Agent 接入可评估**：将来可将有限的本地能力暴露为 MCP Server 或受控本地 API；默认关闭，明确身份/授权，不能把任意 Rust 文件系统命令开放为 Agent 工具。
- **没有 Agent 也完全可用**：搜索、工作区、动态、默认程序打开均保持本地、离线、无登录；不因未来 Agent 引入必须联网、常驻或安装 Node.js 的前置条件。

### 11.2 未来的受控 Tool API（概念契约，非本期开发任务）

未来可从现有 Service 适配出少量、语义明确的业务工具，例如：

| 候选工具 | 对应核心服务 | 权限与风险 |
|---|---|---|
| `resources.search` | SearchService | 仅查询已索引/已授权元数据，不能绕过范围读取全文 |
| `resources.get_metadata` | ResourceService | 保留资源 UUID、来源与权限；不能通过任意传入路径突破授权 |
| `workspaces.list` / `workspaces.get` | WorkspaceService | 默认只读 |
| `workspaces.propose_links` | WorkspaceService | Agent 可提出关联草案，**执行前由用户确认**；不移动原件 |
| `activity.list` | ActivityService | 仅应用实际保存、且有明确来源的时间记录 |

以上命名仅表示设计意图，**不得据此新增 V1 功能、数据库表或不必要的 Agent SDK**。未来开发时应将 Tool Schema、版本、参数校验、业务错误码、超时/取消、幂等键和最小权限统一定义，避免 UI、Rust 和 Sidecar 维护多套不一致实现。

### 11.3 TypeScript Runtime 的可选交付方式

- **V1 不捆绑 Node、Bun、Deno、Pi 或任何 Agent Runtime**。
- 未来确实需要 TypeScript Agent 生态时，优先评估由 Tauri 打包**随应用发布、按需启动**的 Sidecar，而不是让非技术用户手动安装全局 Node/npm。
- 候选路线包括将 TS 应用编译为独立可执行程序（如 Bun compile）、Node SEA，或捆绑受控 Node 运行时和 JS 资源。需对动态导入、原生扩展、子进程、Windows/macOS 架构、安装包大小、签名、公证、升级和内存做实际兼容性 Spike 后再决定。
- Rust 与 Sidecar 之间建议采用明确版本的请求/响应及事件协议（例如 stdio 上的 JSON-RPC 或其他本地 IPC）。Sidecar 崩溃、升级或被禁用，不应损坏核心数据或影响普通搜索。
- Sidecar 按需启动有利于普通使用时减少进程开销，但**不等于安全沙盒**：若 Agent 代码在用户权限下运行，它仍可能访问用户文件。工具白名单和 Rust 网关只能约束合作式工具路径；对不可信扩展/脚本还需要真正的进程隔离与操作系统权限控制。

官方参考：
- Tauri Sidecar：https://v2.tauri.app/develop/sidecar/
- Tauri Node Sidecar：https://v2.tauri.app/learn/sidecar-nodejs/
- Rust MCP SDK：https://github.com/modelcontextprotocol/rust-sdk
- Rig：https://github.com/0xPlaygrounds/rig
- Swiftide：https://github.com/bosun-ai/swiftide

### 11.4 Agent 数据与审计：未来独立聚合

- 将来若启用 Agent，另行设计 `AgentRun`、`AgentMessage`、`ToolInvocation`、`ApprovalRecord`、`ExecutionEvent` 等独立领域聚合和持久化/迁移策略。它们可以通过稳定 `resource_id`、`workspace_id` 引用已有资源，不得要求重建核心关系或依赖路径作为主键。
- 用户可见「动态」可以展示经过筛选的 Agent 操作摘要，**不等于**用通用 `activity_events` 替代完整 Agent 工具调用审计与恢复日志。
- 读取文件正文、向云模型发送内容、写源文件、调用外部服务或执行 Shell 都需要独立风险分级和显式授权；高风险动作默认人工确认。V1 原件只读约束不因本节而改变。
- 对未来的「Agent 可恢复、模型路由、Memory、权限令牌、工具协议和本地推理」只做能力边界记录；必须等实际垂直任务验证后分别设计与验收，不在 V1 提前实现。

## 12. AI 开发维护与正式交付契约（当前工程要求）

### 12.1 工程初始化与日常变更

- 已建立仓库AGENTS.md、README、统一检查与真实构建入口；正式UI在desktop/、原生壳在src-tauri/、领域/数据库在worktwill-core。pnpm check覆盖DTO漂移、TS/Vite、组件交互、Rust测试/格式/Clippy；pnpm bench:search为显式规模验收。原生UI、Office与安装包另行记录，不把构建或mock通过写成产品验收。
- 统一验证入口按项目真实工具覆盖格式/静态检查、TS 与 Rust 编译、跨端契约漂移、领域与 SQLite 测试；原生系统和安装包测试单列环境要求，不能因当前机器无法执行而静默跳过上线门槛。
- 从一个纵向闭环增量开始，每次改动追踪 PRD 的 F/U 场景及所属服务。保持现有职责，不为每个页面新增事实源、hook/store 或重复 API；不因未来 Agent 预装 Sidecar 或框架。
- 选型、支持语义或数据格式的重大决定记录短 ADR，写明问题、依据、代价和状态；与 PRD/架构一致，不另造任务管理系统。Routine 实现选择按既定契约推进，不要求每次可逆编辑都中断等待批准。
- 改动关键测试预期、权限规则、性能预算或恢复行为时，交付说明必须单独解释依据；不能通过删除失败用例、吞错误或修改指标把失败变成通过。范围/行为确需改变时先同步文档和批准记录。

### 12.2 验收层级与可执行约束

| 验收层级 | 必须验证的行为 | 对应契约 |
|---|---|---|
| 领域与数据库 | 归一、重复关联、转移与合并事务、操作重放、删除工作区的历史语义、外键与版本 | F05–F07/F12、U3/U7，第 3 节 |
| 权限与并发 | 撤权中的查询/写入、重叠根/排除/单文件授权、旧扫描不覆盖新事件、异常任务不判删除 | F01/F02/F09，第 5、7 节 |
| 搜索与 UI | 中文短词、范围/排序/分页、输入法、取消竞态、键盘、未授权 Top-N、真实渲染时延 | F03/F10/F14、U1/U4，S03 |
| 原生与办公软件 | 改名/移动/保存、Bookmark/文档身份、默认程序打开、权限与拖入；原件不被应用改写 | F06/F08/F09、U8，S01/S02/S04 |
| 恢复与发行 | 备份、迁移/存储故障、FTS 重建、匹配版本回退、重新授权、干净安装升级 | PRD §9.1，S05，第 7.1、13 节 |

核心用例依据需求固定，并覆盖失败、重试和反例；不能只验证实现内部函数得到了预设输出。先证明关键行为被故意破坏时检查能失败，再把检查当维护保障。修改文案或布局等低影响变更不要求重复运行无关的全量原生实验。

代码审查按需求和状态所有权重新构造反例，避免只复述实现。独立 AI 审查可辅助，但不自动证明正确；首批安装、真实办公保存和恢复必须有真实环境观察证据。代码由 AI 编写不改变原件只读、临时 fixture 和授权范围要求。

### 12.3 交付证据与恢复

- 每次交付列明改动范围、代码版本/构建标识、执行命令、环境、通过/失败/未执行/阻塞及已知限制；尚无版本控制时用快照摘要绑定证据，不虚构 commit。
- 原型展示、纯逻辑 fixture、本机原生调试、真实编辑器与正式签名包分别标识。不能用模拟文件打开证明系统打开、用 Linux/单平台 CI 证明 Win/macOS、用 SQL 时间证明端到端搜索。
- 关键原生与性能报告保留可复现输入和脱敏日志，关联准确构建与支持矩阵。失败记录不覆盖删除，重新验证须说明变化；无新修改或未解问题时不无目的重复全套检查。
- 大改动和迁移保留可回退代码/数据。AI 开发使用测试数据；真实用户元数据、备份与签名凭据不能默认进入提示词、仓库或调试输出。

## 13. 开源与发行工程

2026-11-20用户要求先做好项目工程：Windows/macOS普通本地目录核心闭环及可运行候选优先，本节发行准备安排在核心工程收尾后、相应公开发布前执行。完整备份恢复矩阵与试用后产品观测不作为当前核心收尾前置；已有安全检查和未通过记录保留。当前阶段边界以PRD §9.0及[决定0034](docs/decisions/0034-v1-launch-priorities.md)为准。

- 正式发布前按 PRD §1.4/§10 确认 LICENSE、第三方声明、贡献规则与官方标识；依赖许可证及来源随锁文件检查。当前不替产品选择许可、收费或额外授权系统。
- 锁定 Node/Rust 工具链与依赖，CI 从干净环境构建并保存版本、检查结果和制品摘要。逐步接入依赖漏洞/许可检查，例外有依据与处置，不能将不可运行的检查装饰成通过。
- 选定并验证首发 OS/CPU 架构、最低系统/WebView 依赖与沙盒方式；当前暂缓系统签名/公证，验证实际未签名分发包。未来改变签名或沙盒方式时，重新验证 Bookmark 和权限。
- 当前已选用并实现签名校验的应用内更新，保留手动安装入口。更新须校验制品签名、来源与兼容版本；手动升级同样验收数据保留和失败恢复。
- 签名凭据由受控发布环境提供，不放入源码、模型上下文或普通命令输出。提供可复查构建说明和安全问题报告渠道，不把开源当成已完成安全验收。
- 发布说明包含已验证/有限支持/未支持矩阵、保存与追踪限制、备份恢复入口及已知问题。正式 V1 的 F01–F14、安装、双语核心流程和相关 Spike 不能仅由文档勾选完成。


## 14. v0.6 当前实现与仍需完成项

v0.6记录用户本轮批准的交互变更；021将schema升级为v4，022增加v5文件定位表，024增加v6目录锚点/aliases，025增加v7历史授权来源及修复审计，不是发布证明。`PRD.md` F01–F14/U1–U8与[验收记录](docs/ACCEPTANCE.md)仍是退出条件。

- 代码已落地：Rust原生token/逐项外部导入、单文件与目录授权并集、Pointer Events内部拖入、可关闭公网标题、文件观察双时间、日历聚合/按日分页、工作区排序及完整搜索分页。全局外壳按用户本轮纠正恢复左侧导航与个人工作台默认首页，右侧顶部全局搜索和动态局部日历继续复用既有能力；本轮新布局检查及候选包构建单列记录。
- 仍须按真实执行证据核查：Finder/Explorer拖入和缩放命中、系统默认应用打开、完整WorkTwill退出/重启U8、Word/安全替换/未知身份合并、Windows、正式签名安装与升级恢复。
- 完整搜索已在006加入后端版本化游标与查询读事务，时间线019加入后端版本化末项游标；普通资源列表仍使用offset与前端版本核对；日期/路径字面筛选核心、固定行窗口渲染与 UI 逐项 Link/Transfer 编排已有代码。核心批量命令仍整体事务；大月多文件/深分页、长期数据体积及游标/筛选/窗口渲染的原生性能仍需补齐或验收，不能静默删除原目标。
- 物理身份与平台API候选段落是目标/验证策略；尚未验证完整Bookmark/Document ID连续性。022的活跃独立file grant及024的新目录授权已接入同物理对象Bookmark定位；root原生窗口闭环、Document ID与安全替换仍未闭环。此前本机WPS原位保存/副本+核心进程检查不能提升为所有Office安全保存支持。
- 恢复当前仍沿用 Store 运行会话，但以当前库版本各加一隔离恢复前结果，不是恢复备份中的旧计数；第2.2/7.1的新会话目标仍未落地。验收还须覆盖恢复前查询迟到、前端同步失效、恢复后重新授权及版本计数边界，不能仅凭 session 不变断言权限泄露，也不能仅凭递增代码断言端到端隔离已通过。所有读取/重建和新增命令继续使用统一授权事实源。
- 工作区非文件内容目前仅调研，不新增note/person/task等表，不借视觉重构扩展为任务/笔记系统。2026-11-20已确认Windows与macOS一并首发；核心收尾、特殊目录、发行准备与试用后产品观测的优先级见[决定0034](docs/decisions/0034-v1-launch-priorities.md)。开源许可证和价格仍待决定，未选择许可不公开发布源码。

015备份/恢复复制固定源读快照，Busy/Locked在SQLite等待后明确失败，不持Store gate无限重试；正常More分批继续，最终复制成功才epoch递增，详见[决定0013](docs/decisions/0013-backup-snapshots-and-busy-errors.md)。

本轮布局纠正替代003采用的顶部主导航与动态默认页取舍，旧日志和ADR保持历史内容。个人工作台汇总前3个真实工作区与原有资料面板；不新增领域表、服务、访问历史或权限缓存。026历史授权CASE优化及已有测试证据保留，不因恢复导航而回退；布局决定见[0025](docs/decisions/0025-personal-workbench-sidebar.md)。最终202 Rust/133 UI/API、纽约同组133项及包含新布局的026候选包构建已完成，详见验收记录；原生窗口、Office、Windows和正式发行关口仍未通过。

迭代033针对原生WPS保存延迟增加已知文件的有界快速刷新：通知路径最多256条去重，每2秒至多一个刷新任务，复用checked导入事务及事务guard。只更新已有同身份对象；提交前复核UUID/路径/identity、权限版本、当前未暂停的授权范围及恢复epoch，不建立授权或收录未知对象。未知/移动/删除/替换、通知溢出仍由既有dirty全量扫描核对。checked导入以写锁后复读metadata作为活动与搜索投影的输入。三次本机原生保存约0.7–1.9秒，不代表所有负载或启动补扫SLA；见[决定0030](docs/decisions/0030-bounded-known-file-refresh.md)。

迭代034将通知通道扩展到当前有效、未暂停目录中的新文件。默认跳过目录、用户排除和应用数据边界与Catalog一致；新路径提交时复核竞争发现、当前范围、权限版本及恢复epoch，既有路径仍要求原UUID/identity，不按路径猜测继承关系。checked导入把已复验的当前存在事实记入当前扫描generation，防止旧枚举收尾误判失联，不更改整根完成/进度。详见[决定0031](docs/decisions/0031-authorized-notification-discovery.md)。真实WPS新副本约2.622秒入库是一个本机样本，通知溢出/目录通知/其他平台仍须以全量核对及后续测量为准。

035中同页刷新继续先清除旧overview/page及资源文本，仅由React持有main几何高度作为瞬时交互状态；加载期间的minHeight防止文档高度收缩导致滚动夹回，数据到达或失败后释放，明确导航仍回顶。不延长旧业务DOM的展示或授权有效性，不引入资料缓存，见[决定0032](docs/decisions/0032-reload-layout-height.md)。

036原生Runtime新增至多16个root UUID的FIFO去重队列和固定一个指定目录任务，与原单个全量任务并行；通知元数据刷新仍单个有界任务。新授权、恢复扫描、包含/排除及位置恢复后的idle目录复用入口，溢出仍排全量dirty。Core在位置准备完成、取得roots后且无锁时通知Runtime调度有效未暂停idle目录；已有scan_all默认入口保留。paused位置恢复仍走原已知映射核对，不以扫描新增替代。目录枚举继续使用scan_root，复用授权、身份、generation、恢复epoch和批次写锁；同根偶遇由代次取消过期批次，最多两路枚举，更多资源预算仍需实测。见[决定0033](docs/decisions/0033-bounded-priority-root-scans.md)。

037把暂停根已知映射核对提为与全量流程复用的单根方法，并以refresh_root作为指定维护入口；Runtime继续使用同一队列，暂停位置恢复不再交回全量枚举。只读file_roots的稳定UUID批次100，每批复查状态/身份，不读新增目录项。文件RootRecovery不再用无关全局revision阻止位置回写，仍核目录UUID/path/active/generation/identity/Bookmark/valid、permission_version和原资源UUID/identity/location/归属，并在原checked导入事务复核物理身份与授权。目录位置解析的Anchor.current继续核完整revision。未改变查询、游标或恢复的版本检查，详见[决定0034](docs/decisions/0034-paused-root-maintenance.md)。


2026-11-21 双端合并补充：Mac V1.3.1候选曾用schema9保存便签，Windows候选schema9/10保存连续性；合并版统一schema11。迁移按已知两端结构补齐便签与连续性表，在同一事务校验列并推进版本，保留已有记录和关闭状态；Store沿用升级前在线快照。此前v8→v9描述为各端历史，不代表统一版可以复用版本号。
