# 验收清单与执行证据

**当前收尾入口：** [有限工程剩余清单与完成条件](CURRENT_ENGINEERING_CHECKLIST.md)。优先Mac Finder外拖单项/多项、重复、混合授权、取消与原件不变，再收尾本地核心闭环，单独交付Mac阶段；Windows在用户真机独立构建和验收。038已包含本轮实际分页修复与原生复验，仍不代表Finder外拖通过；公开发行及产品观测另阶段处理。

**038新增结果：** 227 Rust与最终141 UI/API、release构建通过。原生范围/类型/中文路径/日期组合筛选、英文空态与关键词替换、撤权后收藏/日历隐藏及重授权恢复取得证据。修前两次加载更多因stale_cursor回首页，修后最终连续遍历000–102共103个不同文件名；原图坐标点击加载更多后保持094–099、末三项可达且无继续加载。真实内容/权限变更仍清旧页，长间隔观察到重读不升格为无变化游标失效。失败/中间构建/AX与鼠标差异均保留，详见本文末038、[决定0035](decisions/0035-catalog-query-stability.md)和[038原生记录](validation/2026-11-20-native-acceptance-038.json)。Finder外拖仍未通过，未完成Mac阶段交付。

**Git不纳入本次验收：** 用户明确工程完成后自行处理仓库、提交和上传；本阶段交付本地候选、验证结果和已知问题，不执行Git初始化/提交/上传，也不以缺少Git仓库阻塞验收。

**验收顺序补充：** 用户已有Windows电脑，当前先完成Mac原生核心验收并单独交付候选与结果，再对接Windows真机。Windows尚未执行不阻塞Mac阶段交付，仍保留双平台首发及两端独立验收要求；当前不为此安装虚拟机。

**当前执行范围（2026-11-20用户确认）：** 先完成Windows/macOS普通本地目录的工程候选，优先外部拖入、本地保存与重启取回、搜索和权限/原件安全。网络盘/外置盘/云同步目录适配不在V1范围；发行准备、完整备份恢复矩阵及试用后留存/竞品对比后置，不作为当前核心收尾前置。已有备份实现与检查保留，未验收项不改成通过。详见[PRD §9.0](../PRD.md#90-当前工程收尾与后续阶段)及[决定0034](decisions/0034-v1-launch-priorities.md)；以下旧轮次的“必须完成/未完成”按当时范围保留，不自动扩大当前执行目标。

前轮记录（2026-11-20，候选037）；当前本机release包含暂停目录指定维护与快照修复，224 Rust /138 UI/API通过，纽约136项保留029轮次。Computer-Use已恢复，修后内部拖动、有限WPS保存/退出/重启、中文搜索、日历、原生备份及恢复两级取消取得实际证据。033三次真实保存刷新约1.889/1.893/0.734秒，完整V1仍未通过。详情见本文末032/033记录；历史失败和未采用实验保持原始轮次，组件、Core及原生证据不能互相替代。

## 迭代 002：验收、交互修复与第一轮视觉调整

本轮将 `pnpm test:ui` 纳入统一检查。Rust 核心集成测试从 14 项增加到 17 项；新增工作区内保存网页的原子性、删除/移除引用时的源文件与其他关系保留、高命中查询授权与工作区过滤在结果限额之前执行。10 项前端交互测试使用 jsdom 和 mock API，仅证明组件行为。

修正：文件双击重复打开；工作区内添加网页没有关联目标；失败反馈被模态窗口遮挡；删除失败仍导航离开；快速搜索旧响应覆盖风险的回归保护。普通点击选择文件，双击或显式打开按钮发起打开。网页保存与关联在一个 Rust 命令、一个 SQLite 事务中完成。

第一轮视觉调整提高字号与对比度，集中首次使用入口，区分空工作区、空收藏与最近使用状态，搜索分组，提供文件类型、明确操作、扫描中文/英文状态和模态错误反馈。Chrome 中实际查看了空库预览、英文缩放布局及合成资料组件夹具。夹具在开发入口 `/review.html`，与产品入口分开；未连接原生服务，不能用作产品闭环验收。

### Rust 搜索性能

使用正式 `Store::search`、bundled SQLite 3.50.2、磁盘 WAL/FULL 数据库、release 构建。文件名称/路径为合成元数据，没有创建 50 万个真实文件，因此不测扫描耗时或文件监听。15 类查询包括中文短词、混合查询、字面特殊字符、广泛路径命中及无匹配；结果与独立子串匹配/排序逐条核对。

最初 10 万条 P95 116.388ms，违反 60ms 候选预算；保留 [失败记录](validation/2026-11-19-rust-search-before.json)。修复后保留 FTS 低命中查询路径，高命中查询按名称辅助索引分相关性档次读取；每档授权和范围条件均在 LIMIT 之前。没有预先截断候选，也没有提高性能门槛。

| 资源规模/负载 | 样本 | P50 ms | P95 ms | P99 ms |
|---|---:|---:|---:|---:|
| 10,000 | 150 | 3.618 | 4.535 | 4.988 |
| 50,000 | 150 | 1.875 | 5.555 | 7.792 |
| 100,000 | 150 | 2.333 | 10.209 | 16.076 |
| 100,000 + 收藏并发写入 | 150 | 2.842 | 10.216 | 11.636 |
| 500,000 压力规模 | 150 | 6.214 | 37.619 | 38.947 |

总计 750 个计时样本、810 次结果核对，并发期间 61 次业务事务提交成功。100k 后端热查询预算通过。完整数据见 [修复后记录](validation/2026-11-19-rust-search-after.json)。没有测得输入至原生界面可见的 P95，也没有测扫描写入、权限大量变更或真实用户查询集的全部分布。

复现：

```sh
WORKTWILL_TEST_ROOT=/private/tmp/worktwill-spikes-20261119 pnpm bench:search
```

### 真实 WPS 保存

环境为 macOS 15.3.1、APFS、WPS 12.1.26046 (26046)。由脚本在已确认临时目录创建唯一合成 DOCX；正式 Rust 核心将它关联两个工作区并收藏，然后退出观察进程。

通过 CUA 在 WPS 实际打开、输入 `SAVE_ONE_20261119` 并保存；另起核心进程扫描和复验。随后核心观察进程退出期间再次输入 `SAVE_TWO_OBSERVER_CLOSED`、保存、关闭文档、重新打开，画面和 DOCX XML 均包含两个标记。WPS 再将它另存为同目录的独立副本，核心重新扫描。

- 原件 inode 90496413 保持，大小由 995 字节变为 10114 字节；原 UUID、两个工作区关系和收藏保留，定位成功，无人工修复。
- 副本 inode 90502627，独立 UUID，无工作区关系、未收藏。原引用继续指向原件。
- [第一次保存记录](validation/2026-11-19-wps-save-one.json)、[第二次保存和副本记录](validation/2026-11-19-wps-save-copy.json) 为实际核心输出。

这证明本机这一组 WPS 原位保存及另存为操作的核心引用连续性；未证明安全替换、Word、其他目录类型或完整 WorkTwill 窗口 U8。WorkTwill 原生窗口仍遭 CUA 访问拒绝，不能将核心观察进程的退出写成 WorkTwill 窗口退出验收。

## 必须完成的产品关口

| 关口 | 当前证据 | 仍需执行/修正 |
|---|---|---|
| 初次启动、授权、扫描、搜索、系统打开 | 核心临时文件测试；UI 空库视觉检查 | 原生选择器、IPC 与系统默认程序完整操作 |
| 多目录文件关联、重启取回 | 核心改名/重启、重复关联去重测试 | WorkTwill 窗口的实际重启与正确原件打开 |
| 安全移除、多工作区、事务转移 | 核心核对源路径/字节/大小/mtime；UI逐项提交/部分失败报告 | 原生 UI 操作；核心多项命令仍整体事务 |
| 中文/混合搜索、范围、过期响应 | 核心正确性、004最终67项Rust/50项UI回归；完整结果/筛选/版本重读实现 | 版本化游标、取消原生端到端、真实渲染延迟 |
| 网页标题和关联 | 原子保存/关联；自动标题、手填保护/关闭/裸域名、实际公网smoke通过 | 默认浏览器和原生窗口验收 |
| 权限撤销、排除、重叠授权 | 核心过滤与边界测试通过 | 原生授予/撤销后的全部页面验收 |
| 备份、恢复、无效备份 | 核心磁盘测试、连续恢复版本递增、旧schema暂存迁移通过 | 原生保存/选择器、恢复结果反馈、升级失败恢复 |
| 办公保存 U8 | 本机 WPS 原位保存/另存为 + 核心重启观察通过 | 完整窗口 U8、Word、安全替换和平台支持矩阵 |
| 外部与应用内拖拽 | 原生token/物理身份/授权及Pointer Events实现；取消/迟到/目标范围组件回归通过 | **Finder/Explorer与原生缩放命中未执行，不能算真机通过** |
| 日历和文件变化时间线 | 观察事实、双时间、选日跨月、当前范围/授权、分页实现；核心5+UI11通过 | 原生外部修改到窗口刷新、大月预算 |
| 设计细化、中英及窗口适配 | 新版白/冷灰/蓝，日历三栏、顶部导航；中英100%–200%实际视觉检查 | 原生窗口/输入法/全部对话框英文与系统缩放 |
| Windows、签名安装、正式发行 | 未执行 | 保持未验证，不能自动登记通过 |

原生 UI 工具阻塞原文为 `Computer Use was not approved to use WorkTwill`。这是当前无法执行完整窗口验收的外部限制；独立可执行的实现、核心测试和其他应用实测继续推进。

## 迭代 003：用户新增方向

用户于本轮明确批准：原生/应用内拖拽为必需能力；URL 自动取可编辑标题；参考日历时间线图片细化整体设计与色调；动态优先展示文件修改与首次收纳，降低工作区管理事件的权重。该授权取代旧版 PRD 对“网页不自动取标题”和动态呈现的对应限制，具体协议和验收将在实施前补齐。

子 agent 调研白领工作区的非文件内容，只形成建议和依据；不自动把调研中的任务、笔记或 Agent 功能全部纳入 V1。完整范围与原有未完成关口继续保留。

### 迭代 003 最终实施与复核（2026-11-20 00:05）

用户四项方向均已进入正式代码，文件/关系数据继续由Rust持有，未以浏览器演示替换产品后端。源码和二进制对应 [最终候选指纹](validation/2026-11-20-v1-candidate.json)。

1. **拖入与授权**：原生DropInbox、300秒token、后台捕获物理身份、提交前复验、固定目标/防并发/过期/取消/部分成功；单独文件授权不扫描邻居。Pointer Events内部拖动支持多选、阈值、高亮、Esc/失焦取消。准备阶段可取消。松手先发started固定目标，慢盘期间切换工作区不改目标；旧ready或无started回包不关联。界面完成后丢弃短时凭据。
2. **网页**：URL优先，裸域名补HTTPS、hostname即刻填标题，600ms后获取；晚回包不覆盖手填。关闭自动标题/失败/超时可保存。Rust统一DTO，5秒/3跳/256KiB/200字符和逐跳公网校验。原子保存+关联、编辑、收藏及全局删除确认完整。网络smoke为独立请求，非原生窗口端到端。
3. **视觉和使用闭环**：白底冷灰蓝、顶部导航、日历/时间线/快捷返回；首次动态页直接授权；精确搜索/分类空态、日期/路径前缀筛选、工作区排序、排除项恢复。>100资源用84px固定行高/16行窗口渲染。批量关联/转移逐项调用正式mutation并展示成功/失败，失败来源保留；核心多项命令自身仍整体事务。
4. **时间线**：Catalog与单文件复验产生首次发现/真实修改，重复扫描不重复事件，文件mtime与观察时间分开。日历本地日边界/跨年/DST，事件与月统计同筛选同权限，版本变化重读首页。工作区列表重读的暂时空态不清掉已选范围；真正删除后才重置。
5. **重新指向补强**：原生picker捕获一次身份，合并确认继续用该快照；目录外修复可仅授所选文件，旧对象授权不转给另一identity；合并active授权取并集，不复活已撤销授权。同路径替换及捕获失败后出现文件拒绝。
6. **响应隔离**：变更通知同步失效loadSequence与searchSequence并清资源文本；扫描轮询迟到回包不能恢复旧数据；完整搜索改词/范围/筛选重置分页；工作区结果不可当文件批量关联；完整搜索页新增网页不继承隐藏的来源工作区。

### 最终自动检查

完整检查退出码0，原始输出见 [check日志](validation/2026-11-20-check-v1-003.log)。

| 层级 | 结果 | 证据边界 |
|---|---:|---|
| Rust领域/原生单测 | **56通过** | timeline5、workflows25、drop identity4、relink grants8、search filters4、native token6、URL4；临时真实文件/磁盘库与原生纯逻辑，不含真实窗口 |
| React交互 | **42通过** | mock API/jsdom，含迟到拖入/目标切换/取消、标题保护、分页/筛选、失效响应、虚拟列表、范围保留 |
| America/New_York再验 | **42通过** | 同一组UI回归，验证本地日历/DST日期构造；[时区日志](validation/2026-11-20-ui-new-york.log)，不重复计算为新增42项独立测试 |
| DTO/TS/Vite/rustfmt/Clippy | **通过** | 正式workspace/全部target，Clippy -D warnings |
| release macOS arm64构建 | **通过** | [构建日志](validation/2026-11-20-build-release.log)，优化构建约16.05MiB；编译器ad-hoc，未Developer ID签名/公证 |
| 原生窗口、Finder、IPC完整操作 | **阻塞，未执行** | 2026-11-20对最终release `.app` 再次访问，CUA返回 `Computer Use was not approved to use WorkTwill` |

本轮最终搜索复测因新增单文件可见性、31条has_more和分页/筛选变化而执行；此前失败和修复后记录保留，不覆盖。正式 `Store::search` 最终结果见 [003性能JSON](validation/2026-11-19-rust-search-v1-003.json)：

| 规模/负载 | 样本 | P50 ms | P95 ms | P99 ms |
|---|---:|---:|---:|---:|
| 10k |150|3.642|4.480|4.754|
| 50k |150|1.864|5.637|5.949|
| 100k |150|2.333|9.186|9.490|
| 100k并发收藏写入 |150|2.752|9.390|10.173|
| 500k压力 |150|6.374|39.383|41.482|

750计时样本，810次独立结果核对，83次并发业务提交。此项是合成元数据磁盘查询，不提升为原生输入可见延迟、真实扫描或全部负载支持。

### 视觉核查和调研

Chrome实际查看正式App+mock IPC的独立review入口，保持“视觉验收夹具/合成资料/不连接原生后端”标识。确认新版日历三栏、选日过滤、中英文导航/资料列表、100%/150%/200%缩放、自动标题弹窗及完整搜索日期路径控件排版。原生Finder拖入和默认软件没有通过夹具假装验收。独立复核见 [时间线/界面审查](research/TIMELINE_UI_REVIEW.md)。

[白领工作区调研](research/WORKSPACE_CONTENT_RESEARCH.md)比较Notion、Anytype、Capacities、TagStudio官方能力；本轮保持文件+网页正式资源，简短说明为未来候选，不加入联系人/任务/独立笔记引擎。

### 交付与尚未关闭的关口

- 应用：`target/release/bundle/macos/WorkTwill.app`。
- 可携带压缩包：`deliverables/WorkTwill-v0.1.0-v1-candidate-macOS-arm64.zip`。无演示数据，不需Node/Rust运行环境，不自动安装到Applications。
- 操作：[使用说明](USER_GUIDE.md)；实施：[PRD v0.6](../PRD.md)、[架构v0.6](../ARCHITECTURE.md)、[决定0002](decisions/0002-v1-drag-web-timeline.md)。
- 原生授权→扫描→搜索→拖入→打开→退出/重启→撤权→备份恢复，完整U8、Word/安全替换、Windows、签名安装/升级仍未通过。后端执行取消、版本化游标、分组配额、长期事件/大目录预算和原生可见时延仍保留在退出条件，不静默降低为通过。

因此这是**可复查的V1验收候选交付**，尚不能标成全部V1验收完成或商业发布可用。解除WorkTwill工具访问拒绝后，应先执行上述原生关口，再决定是否满足V1退出条件。

## 迭代 004：搜索运行中取消与混合分类交付（2026-11-20）

本轮继续完成原F03/F14中独立可执行的缺口，没有修改V1退出标准。决定见 [0003](decisions/0003-search-cancellation-and-groups.md)。

- **后端真正取消**：每查询独立SQLite连接，progress_handler中断正在执行的VM；预先取消、读锁后取消、组装/返回前检查。取消返回search_cancelled，不伪装为空结果。旧非取消Store入口仍可用。
- **原生查询隔离**：begin_search_session产生UUID，quick/page两个固定槽，新请求只取消同lane旧请求；取消先到的水位不丢失，旧session/ID不能重新抢占。API模块初始化一次并维持跨组件递增ID；WebView重载新建会话。关闭/删改快速搜索或完整页改筛选/离开时分别取消。取消自身不新建会话，初始错误向调用者返回。
- **混合快速结果**：all且匹配>30时，每类至多预留10项；不足余位按全局相关性补齐。授权/范围/类型始终在limit之前。分类内顺序、显式类型与完整搜索仍遵循原全局排序；新增派生复合索引，schema版本仍3。相应代价是混合快速选集不再承诺等于不分类型的Top30。
- **可见交互**：面板展示各类计数并可跳转；跳转同步键盘高亮，随后Enter不再打开旧分类。Chrome实际检查三类结果及网页选中反馈；仍是标识明确的视觉夹具，不是原生服务。

### 最终检查和性能

[统一检查日志](validation/2026-11-20-check-v1-004.log)退出码0：**67项Rust、50项UI**、DTO/TS/Vite/rustfmt/Clippy通过。[纽约时区](validation/2026-11-20-ui-new-york-004.log)同组50项通过，不重复计为新增测试。真实运行中取消测试等VM callback执行后再取消，确认OperationInterrupted及后续查询/其他连接可用；Core两个正式入口均确实触发VM中断。新增公开接口2项、native session/lane3项、分类席位3项及API/组件交互回归。

为避免使用旧性能数字，在以上变更后顺序执行两个release基准，没有并行构建或测试干扰计时。原始 [规模报告](validation/2026-11-20-rust-search-v1-004.json)、[混合报告](validation/2026-11-20-rust-search-mixed-v1-004.json) 和 [执行日志](validation/2026-11-20-search-bench-004.log)均保留。

| 规模/负载 | 计时样本 | P50 ms | P95 ms | P99 ms |
|---|---:|---:|---:|---:|
| 10k文件元数据 |150|3.831|5.577|5.654|
| 50k文件元数据 |150|2.326|6.039|6.135|
| 100k文件元数据 |150|2.841|9.742|9.946|
| 100k并发收藏写入 |150|3.114|9.834|10.670|
| 500k文件压力 |150|6.672|37.631|40.157|
| 100k混合资料 |220|2.939|10.516|15.025|

混合库包含98900文件/1000网页/100工作区和400未授权文件诱饵；22案例覆盖中文短词、英文、符号、路径、URL、类型/工作区筛选、某类不足10后的余位分配、少量及无匹配。全量内存oracle不调用生产候选算法；242次核对全部一致，每查询P95也低于原60ms后端预算。两基准总970个计时样本、1052次独立结果核对；74次并发写事务。单查询仅10个热样本，尾部分位数不作为稳定生产SLA。

以上是合成元数据、磁盘WAL/FULL、正式Store::search的证据，不是原生可见延迟、真实大目录扫描或用户查询集全部分布。

### 更新候选与未关闭关口

优化release构建通过，约16.11MiB，[构建日志](validation/2026-11-20-build-release-004.log)。最新包为`deliverables/WorkTwill-v0.1.0-v1-candidate-004-macOS-arm64.zip`，003原压缩包保留，当前`target/release/bundle/macos/WorkTwill.app`为004。对应[004源码/二进制指纹](validation/2026-11-20-v1-candidate-004.json)。

再次访问004原生窗口仍返回`Computer Use was not approved to use WorkTwill`，未绕过应用访问限制。原生拖入/选择器/IPC/默认程序/完整退出重启和U8、Windows、Word/安全替换、签名安装等关口保持未执行。版本化游标、大月/长期事件预算、真实渲染及扫描资源预算也仍保留，不借取消与分类完成宣称全部V1通过。


## 2026-11-20 迭代005：大月日历响应边界

### 实施与独立复核

针对用户要求的文件日历，实际测得旧月历逐精确时间点返回的数据量随活动条数增长。改为必填本地午夜边界，Rust严格校验、同事务同权限按日汇总，最多31个点。事件列表仍100条分页，mtime/观察时间与首次发现语义不变；选日不截断整月日期计数。使用每日索引范围计数与有效时间派生索引，旧v3和恢复暂存库打开时幂等创建。决定见[0004](decisions/0004-bounded-local-calendar.md)。

独立agent只读复核了SQL参数、授权/工作区/DST及迁移入口，未发现新的确定缺陷；其指出的证据边界继续保留。新增混合多资源/网页/未授权文件、跨月、工作区/双时间检查，以及非法边界、23/25小时日、闰月检查。Chrome实际选日再返回整月，确认明细缩小而月份计数保留；使用正式App+mock IPC合成夹具，不作为原生或真实后端证据。

### 统一检查与规模实测

最终[统一检查](validation/2026-11-20-check-v1-005.log)通过：生成DTO、TS/Vite、rustfmt、Rust workspace、Clippy `-D warnings`；**70项Rust、51项UI/API**。另在[纽约时区同组51项](validation/2026-11-20-ui-new-york-005.log)通过，明确断言春季23小时、秋季25小时。3项ignored规模基准另行显式执行，不计入70项通过。

时间线使用同一个授权文件、同一月唯一时间戳的100k/500k合成修改，正式release磁盘Store调用。每规模10个热样本，含首100事件卡片和整月计数，序列化体积在计时外测量。基线、变慢的中间CASE方案和最终日志全部保留：

| 活动规模 | 原日历点/响应字节 | 原P95 ms | CASE方案P95 ms | 最终点/响应字节 | 最终P95 ms |
|---|---:|---:|---:|---:|---:|
| 100k | 100000 / 3011119 | 150.859 | 185.622 | 30 / 52104 | 124.855 |
| 500k | 500000 / 14854189 | 754.061 | 922.193 | 30 / 52134 | 624.696 |

[基线JSON](validation/2026-11-20-rust-timeline-before-005.json)、[CASE中间JSON](validation/2026-11-20-rust-timeline-case-005.json)、[最终JSON](validation/2026-11-20-rust-timeline-v1-005.json)及对应before/case/final日志在validation目录。核对整月总数、逐日独立预期分布、100条明细与响应体积上限。500k仍约625ms；未测多文件月、深offset、首次建索引成本、长期数据库体积或原生显示，不能据此宣称全部时间线性能达标。

新增索引后顺序重跑两项搜索release基准，无并行构建干扰。最新[文件元数据报告](validation/2026-11-20-rust-search-v1-005.json)、[混合报告](validation/2026-11-20-rust-search-mixed-v1-005.json)及[执行日志](validation/2026-11-20-search-bench-005.log)：

| 查询规模/负载 | 计时样本 | P50 ms | P95 ms | P99 ms |
|---|---:|---:|---:|---:|
| 10k 文件元数据 | 150 | 4.513 | 6.351 | 6.699 |
| 50k 文件元数据 | 150 | 2.468 | 6.698 | 7.129 |
| 100k 文件元数据 | 150 | 3.112 | 10.300 | 10.746 |
| 100k 并发收藏写 | 150 | 3.352 | 10.256 | 11.093 |
| 500k 文件元数据 | 150 | 6.728 | 38.105 | 38.963 |
| 100k 混合资料 | 220 | 3.022 | 11.246 | 13.576 |

两项搜索合计970计时样本、1052次独立oracle核对，81次并发写提交。100k后端热查询保持原60ms预算；不代表输入至原生结果显示、真实扫描并发或空闲资源预算。

### 005交付和未通过关口

`target/release/bundle/macos/WorkTwill.app`为本轮优化release，约16.09MiB；[005压缩包](../deliverables/WorkTwill-v0.1.0-v1-candidate-005-macOS-arm64.zip)约6.17MiB，ZIP CRC及必需bundle条目通过。正式dist不含review入口或合成标识。含编译器ad-hoc签名，没有Developer ID/公证，不自动安装到Applications。构建日志和[005指纹清单](validation/2026-11-20-v1-candidate-005.json)保留；004旧包和所有历史证据未覆盖，没有Git提交或推送。

本轮再次通过CUA访问最终005应用，仍返回`Computer Use was not approved to use WorkTwill`。没有改名应用或用其他GUI桥接绕过。Finder拖入、原生选择器/真实IPC/默认软件/完整退出重启和U8仍未执行；Windows、Word/安全替换、签名安装、版本化游标、扫描与原生渲染资源预算等关口继续保留。005完成了本轮日历规模迭代，不等于完整V1验收完成。


## 2026-11-20 迭代006：搜索分页的版本约束与读取快照

### 行为与审查

完整搜索后续页现在必须回传后端返回的next_cursor及next_offset。游标包含协议版本、Store session、权限/revision及规范化关键词、工作区、类型、排序、日期和路径前缀；缺失或不匹配返回stale_cursor，不能先读取后再由前端猜测是否可拼接。第一页不带游标，最后一页不返回后续游标。快速与完整搜索的候选、资源详情和版本在同一SQLite读取事务中组装，保留执行取消和两条查询槽。见[决定0005](decisions/0005-versioned-search-pagination.md)。

React按有效响应的next_offset续页；后续页失效时清掉旧结果、游标和选择并重读首页，首页失败不循环重试。迟到的旧分页在更新资料和cursor之前被序号检查拒绝。独立只读审查未发现新的确定缺陷，并明确游标是客户端回传的版本化描述，不是签名或授权凭证；SQL仍每页独立授权，不持有跨请求快照，内部仍是OFFSET。

### 最终检查与性能

[统一检查](validation/2026-11-20-check-v1-006.log)通过DTO、TS/Vite、rustfmt、Rust workspace与Clippy全部告警检查，**76项Rust、54项UI/API**通过；[纽约时区同组54项](validation/2026-11-20-ui-new-york-006.log)也通过。新增核心5项游标场景及1项实际独立连接提交下的读取快照检查，UI新增3项分页恢复/错误/旧回包回归。保留全部原有测试，原分页用例改为回传真实响应游标。

覆盖三种排序完整三页去重、规范化等价条件、所有筛选维度与版本/偏移变化、缺游标、资料变更、撤权、重启、同备份连续恢复，以及首页失败不循环、缺后续游标不发任意offset请求、旧分页不能污染新cursor。读取快照检查在真实版本SQL执行期间由另一SQLite连接提交收藏和版本变化，核对返回的资料、权限与revision属于原快照。

读取事务变化后顺序重跑两项release搜索基准，无并行构建干扰。[文件报告](validation/2026-11-20-rust-search-v1-006.json)、[混合报告](validation/2026-11-20-rust-search-mixed-v1-006.json)、[原始日志](validation/2026-11-20-search-bench-006.log)：

| 查询规模/负载 | 计时样本 | P50 ms | P95 ms | P99 ms |
|---|---:|---:|---:|---:|
| 10k 文件元数据 | 150 | 3.723 | 5.486 | 5.757 |
| 50k 文件元数据 | 150 | 2.208 | 6.152 | 6.251 |
| 100k 文件元数据 | 150 | 2.678 | 9.483 | 9.668 |
| 100k 并发收藏写 | 150 | 2.943 | 9.756 | 9.884 |
| 500k 文件元数据 | 150 | 6.527 | 37.407 | 38.289 |
| 100k 混合资料 | 220 | 2.847 | 10.157 | 13.498 |

搜索共970计时样本、1052次独立oracle核对，59次并发写提交；100k后端热查询保持原60ms预算。这是合成元数据磁盘快速查询性能，未测新增游标的深分页或原生可见时延。时间线规模证据沿用005记录，本轮未修改时间线查询路径，未将其重新计为新执行。

### 006交付与未通过关口

本轮release应用位于`target/release/bundle/macos/WorkTwill.app`，约16.13MiB；[006压缩包](../deliverables/WorkTwill-v0.1.0-v1-candidate-006-macOS-arm64.zip)约6.18MiB。ZIP CRC及必需bundle条目通过，正式dist不含review入口或合成夹具标识。`codesign -dv`显示arm64、adhoc/linker-signed、TeamIdentifier未设置；没有Developer ID签名、公证或资源封装签名，不自动安装到Applications。[构建日志](validation/2026-11-20-build-v1-006.log)和[006源码/包体指纹](validation/2026-11-20-v1-candidate-006.json)保留，未覆盖005旧包，无Git提交或推送。

再次通过CUA访问最终006应用，仍返回`Computer Use was not approved to use WorkTwill`，没有绕过限制。原生选择器/真实IPC/Finder拖入/默认软件/完整保存退出重启与U8仍未执行；Word/安全替换、Windows、签名安装、原生可见性能、扫描资源预算及长期数据等关口继续保留。搜索游标和读取快照完成是本轮进展，不代表完整V1或商业发布验收完成。


## 2026-11-20 迭代007：实际文件扫描、投影复用与提交复验

### 测得问题与修复

10k实际合成文件、100子目录的正式release扫描基线：首次5.636秒；不变重扫并发搜索8.900秒，查询P95 91.491ms；数据库由29,347,840增至35,098,624字节。此前每文件重写两套FTS，且同路径检查没有location索引。文件创建时间独立记录，不算扫描耗时。

现由search::project比较规范化kind/name/location及两套FTS行存在性，内容不变时不写投影；缺映射或FTS行仍修复，显式重建仍完整重建。新增非唯一resources(location)派生索引，保留同路径旧UUID。仍执行资源、身份、代次、授权及观察事件流程，不称文件系统差量枚举。FTS行存在但内容错误依赖显式重建，未测FTS底层损坏。

新增确定性回归在原提交边界失败：旧批次将43字节写回11字节，[原始失败日志](validation/2026-11-20-stale-scan-before-007.log)保留。现提交前复读metadata/身份/canonical path，使用当前mtime/size，不由旧捕获值覆盖更新。候选消失或路径换链接时拒绝提交并累计failures到扫描循环。独立只读复核未发现确定新问题；复验不是原子句柄操作，检查后的文件系统变动窗口仍存在。见[决定0006](decisions/0006-scan-projection-and-current-metadata.md)。

### 最终验证与扫描测量

[统一检查](validation/2026-11-20-check-v1-007.log)通过DTO、TS/Vite、rustfmt、Rust workspace及Clippy全部告警检查：**81项Rust、54项UI/API**；[纽约时区同组54项](validation/2026-11-20-ui-new-york-007.log)通过。新增2项投影测试、3项提交边界测试；保留重命名、同路径替换、重叠根、撤权/排除、单文件及备份重建回归。常规4项ignored基准不冒充执行，本轮另显式运行扫描和两项搜索，时间线沿用005证据。

最终10k文件扫描测量已包含当前metadata及canonical复验；[基线](validation/2026-11-20-rust-scan-before-007.json)、[仅投影优化中间结果](validation/2026-11-20-rust-scan-projection-007.json)、[最终结果](validation/2026-11-20-rust-scan-v1-007.json)及对应日志均保留。

| 阶段 | 基线耗时 s | 最终耗时 s | 基线并发搜索P95 ms | 最终并发搜索P95 ms |
|---|---:|---:|---:|---:|
| 首次扫描 | 5.636 | 2.530 | 未测 | 未测 |
| 不变重扫 | 8.900 | 0.624 | 91.491 | 5.788 |
| 一次修改后重扫 | 8.951 | 0.634 | 94.206 | 5.384 |

最终不变重扫61个并发查询样本，一次修改63个；尾部样本量有限，不当稳定SLA。核对10k资源、10k首次发现、仅1次修改、同一UUID/工作区/收藏保留及样本文件内容；根状态completed、扫描数10k、失败0。最终数据库30,588,928→30,633,984字节，重扫增长45,056字节。初始库比基线更大；新增索引有存储和首次创建成本，未由本次空库扫描推定旧大库迁移成本通过。

消失候选和父目录链接测试直接调用commit_batch，证明提交拒绝和失败计数；扫描收尾partial_failure/跳过missing由源码路径复核，未当成原生完整流程实证。当前仍完整枚举元数据，100k实际文件、事件洪峰、原生可见时延、空闲CPU/RSS、云目录及Windows继续保留。

### 搜索复测

投影和索引变更后顺序重跑两个release基准，无并行构建干扰。[文件报告](validation/2026-11-20-rust-search-v1-007.json)、[混合报告](validation/2026-11-20-rust-search-mixed-v1-007.json)、[原始日志](validation/2026-11-20-search-bench-007.log)：

| 查询规模/负载 | 样本 | P50 ms | P95 ms | P99 ms |
|---|---:|---:|---:|---:|
| 10k 文件元数据 | 150 | 3.676 | 5.259 | 5.688 |
| 50k 文件元数据 | 150 | 2.151 | 5.725 | 6.131 |
| 100k 文件元数据 | 150 | 2.789 | 9.393 | 9.863 |
| 100k 并发收藏写 | 150 | 2.916 | 9.524 | 10.064 |
| 500k 文件元数据 | 150 | 6.221 | 36.709 | 37.753 |
| 100k 混合资料 | 220 | 2.936 | 9.838 | 12.866 |

合计970计时样本、1052次独立oracle核对、54次并发提交；100k后端热查询保持原60ms预算。该搜索规模数据是合成元数据磁盘查询，不等同于同规模真实文件扫描或原生UI。

### 007交付与未通过关口

最终release应用`target/release/bundle/macos/WorkTwill.app`约16.13MiB；[007压缩包](../deliverables/WorkTwill-v0.1.0-v1-candidate-007-macOS-arm64.zip)约6.18MiB，ZIP CRC及必需bundle条目通过。正式dist不含review入口/合成标识。[构建日志](validation/2026-11-20-build-v1-007.log)、[源码与包体指纹](validation/2026-11-20-v1-candidate-007.json)保留。codesign显示arm64 adhoc/linker-signed，TeamIdentifier未设置、无资源封装签名；没有Developer ID/公证，不自动安装到Applications。旧包与历史失败证据保留，无Git提交或推送。

本轮再次访问最终007原生应用，CUA仍返回`Computer Use was not approved to use WorkTwill`。没有绕过限制。原生拖入/选择器/IPC/默认程序/完整保存退出重启与U8仍未执行；Word/安全替换、Windows、签名发行、原生性能、空闲资源、大规模真实扫描与洪峰等关口继续保留。007完成本轮扫描优化与提交复验，不将其登记为完整V1通过。

## 2026-11-20 迭代008：加载与草稿状态、原生确认语言和长路径布局

### 行为与界面复核

资料读取中优先显示加载状态，不短暂显示搜索/分类空结果；status 与 aria-busy 同步，隐藏空态操作。设置页排除目录按 root UUID 保存受控草稿，请求失败保留、成功只清对应根且仍等于提交值的输入，刷新 overview 不抹掉其他草稿；沿用页面统一 busy 和 mutate。

新增 Rust UiLanguage DTO 并生成 TS。选择目录、选择文件、重新指向、备份、恢复与拖入提交六入口显式传当前语言；长驻拖入监听读取最新语言。原生授权/重新指向/独立合并/恢复的标题、正文、确认和取消按钮提供中英文本；不改变授权顺序或恢复机制。系统文件选择器按钮随操作系统语言，后端错误尚未全部翻译。见[决定0007](decisions/0007-ui-language-and-input-states.md)。独立只读复核确认消费者覆盖、按钮语义与警示范围，没有把本地化当成原生执行证据。

Chrome 正式 App + mock IPC 夹具暴露了单独授权卡片的长路径溢出：文本是第一个子元素，原 nth-child(2) 规则没有作用；DOM 与计算样式显示路径宽度1534.53px。改为明确 location-copy，并对路径允许任意换行。英文设置页100%与200%实际截图复核，180字符单段路径能在卡片内换行，Revoke 保持可见。夹具持续显示合成标识，不连接原生后端。

### 检查与交付

[统一检查](validation/2026-11-20-check-v1-008.log)通过 DTO、TS/Vite、rustfmt、Rust workspace、Clippy全部告警检查；**81项Rust、62项UI/API**通过。[纽约时区同组62项](validation/2026-11-20-ui-new-york-008.log)也通过。新增8项UI/API回归覆盖六入口语言参数、语言切换后拖入闭包、排除失败保留及成功只清当前草稿、搜索/文件/网页加载态。保留原有拖入、分页、日历和权限测试。本轮未改搜索/扫描/时间线查询路径，规模数据沿用007/005，未将其计作本轮新执行。

最终release应用位于`target/release/bundle/macos/WorkTwill.app`，约16.13MiB；[008压缩包](../deliverables/WorkTwill-v0.1.0-v1-candidate-008-macOS-arm64.zip)6,483,264字节。ZIP CRC与必需bundle条目通过；正式dist不含review或合成夹具标识。[构建日志](validation/2026-11-20-build-v1-008.log)和[008源码/包体指纹](validation/2026-11-20-v1-candidate-008.json)保留。arm64 adhoc/linker-signed，TeamIdentifier未设、无资源封装签名；没有Developer ID或公证，不自动安装到Applications，无Git提交或推送。

访问最终008原生应用，工具仍返回`Computer Use was not approved to use WorkTwill`。没有绕过限制。原生选择器/IPC/Finder拖入/默认程序/完整退出保存重启与U8继续未执行；Word/安全替换、Windows、签名发行、原生可见性能及大规模实际扫描等关口保留。008是本轮可复核候选，不是完整V1通过。

## 2026-11-20 迭代009：100k实际文件、初扫读取并发与恢复代次

### 补测揭露的问题

扩展既有ignored扫描验收，可显式选择10k或100k文件；默认仍10k，只在确认临时目录新建自己的TempDir。100k小文本文件分布于1000子目录。第一轮顺序初扫61.864秒；随后加入初扫并发查询，观察到P95 170.539ms/P99 490.958ms。早期脚本仅末尾核对汇总，不能由此证明各阶段正确或严格扫描中已可查；原报告与日志保留，未将它登记为性能通过。

经独立复核补齐oracle：每个阶段分别核对N资源、N个不同资源的首次发现、预期修改事件及目标UUID、根完成/扫描数量/零失败；比较全部资源UUID、路径、名称、身份、mtime、size、状态，预期只有目标文件metadata改变。初扫后核对全部预期路径；目标原件字节和工作区/收藏另查，不宣称逐一验证所有其他文件字节。查询前后确认根仍running且尚未扫描完，排除结束边界的调度空隙。报告区分总查询与最多前10k保留时延，含已开始的在途尾查询；本轮各阶段均低于上限，无截断。

严格oracle旧锁基线见[报告](validation/2026-11-20-rust-scan-100k-oracle-before-009.json)与[日志](validation/2026-11-20-scan-100k-oracle-before-009.log)：初扫63.956秒，查询P95/P99 128.036/200.572ms。原因是Catalog每100条持gate独占锁，覆盖FTS写入和提交，搜索read等待整批写入。

### 最小正确改动与回归

Catalog批次改为catalog_write Mutex→共享gate→BEGIN IMMEDIATE；独立mutex串行同Store批次，gate继续保护恢复/授权隔离，SQLite WAL读取可并发。setup/finalize及其他控制写入仍独占gate；资源、身份、FTS、活动和revision仍在同一事务，当前metadata/路径提交复验保留。overview/resources/file_grants补齐单读事务，避免旧候选、新字段与版本混合。

独立审查还发现恢复的generation ABA：旧扫描10，恢复备份8变9，重授权再变10，旧批次会错误通过。新增不从备份恢复的进程内scan_epoch，扫描从开始、检查、提交到收尾均核对；恢复成功后在独占gate释放前递增。见[决定0008](decisions/0008-scan-read-concurrency-and-restore-epoch.md)。

[旧代码回归失败日志](validation/2026-11-20-scan-lock-and-aba-before-009.log)真实包含“读租约阻止批次提交”和“恢复后旧代次复活”两失败；修复后通过。新增overview/resources测试在正式版本SQL准备时协调另一真实SQLite连接提交，返回内容和版本仍是旧快照，下一次读取显示新版本；测试hook仅cfg(test)，不进入生产构建。独立最终只读复核未发现确定缺陷，不当作执行测试。

[完整检查](validation/2026-11-20-check-v1-009.log)通过DTO、TS/Vite、rustfmt、Rust workspace、Clippy全部告警检查：**85项Rust、62项UI/API**。UI未改，纽约时区62项沿用008，不重复计为009执行。显式扫描基准另行release运行；常规4项ignored不冒充运行。纯搜索规模沿用007，时间线沿用005，本轮未改其选择/聚合路径。

### 最终100k测量

[最终报告](validation/2026-11-20-rust-scan-100k-v1-009.json)、[日志](validation/2026-11-20-scan-100k-final-concurrency-009.log)。每次查询完成后等待5ms，查询为高命中二字中文“合同”。这是固定思考时间的闭环负载；响应变快时查询次数也会增加，不能把扫描墙钟差异只归因于锁开销。

| 阶段 | 旧扫描耗时 s | 新耗时 s | 旧查询P95/P99 ms | 新查询P95/P99 ms | 新查询样本 |
|---|---:|---:|---:|---:|---:|
| 首次扫描 + 并发搜索 | 63.956 | 71.456 | 128.036 / 200.572 | 2.921 / 13.896 | 7221 |
| 不变重扫 + 并发搜索 | 10.165 | 10.380 | 12.833 / 25.836 | 3.127 / 5.279 | 1178 |
| 一文件修改后重扫 | 8.442 | 8.663 | 9.361 / 13.122 | 3.594 / 6.091 | 972 |

最终9371查询，逐阶段oracle全部通过；初扫约24ms观察到30条结果，前后根均running且scanned=100/100000。首扫样本从1003增至7221，总扫描耗时增加约11.7%，这是本次观察到的主要代价；单轮数据不当稳定SLA。文件生成6.873秒不计扫描；重复扫描前后主库304,947,200→305,070,080字节，增长122,880字节。不宣称文件系统差量枚举、冷磁盘、真实办公目录、原生窗口响应或空闲CPU/RSS通过。

### 009交付与关口

最终本机release应用位于`target/release/bundle/macos/WorkTwill.app`，压缩包见[009候选](../deliverables/WorkTwill-v0.1.0-v1-candidate-009-macOS-arm64.zip)，[构建日志](validation/2026-11-20-build-v1-009.log)和[源码/包体指纹](validation/2026-11-20-v1-candidate-009.json)保留。仍为macOS arm64、编译器ad-hoc签名，不含Developer ID/公证，不自动安装到Applications；旧包、早期报告及失败日志保留，无Git提交或推送。

原生窗口仍受工具访问限制；原生选择器/IPC/Finder拖入/默认程序/完整Office保存退出重启与U8、Word/安全替换、Windows、签名安装、真实用户重复使用等继续保留。100k后端场景完成不等于完整V1或商业发布验收完成。

## 2026-11-20 迭代010：首次常用位置与授权意图

对照PRD F01补齐桌面、下载、文档建议。首次动态、空资料列表与无有效授权的设置页复用受控组件，加载管理范围时不提前显示，已有目录或单文件授权时隐藏。三个建议由Rust标准目录API设置系统选择器起点，前端不传任意路径；用户可在选择器中更换目录，只有最终确认的选择结果才grant_root。取消不授权、不通知变化、不启动扫描，解析失败明确报错，通用添加管理位置入口保留。见[决定0009](decisions/0009-suggested-managed-locations.md)。

新增SuggestedLocation DTO并重新生成TS。没有新建路径授予命令、提前读取用户目录或创建默认目录。独立只读复核核对了Tauri/dialog/rfd路径：set_directory只设置原生面板起点，不预授权限；源码复核不当作选择器操作通过。

[统一检查](validation/2026-11-20-check-v1-010.log)通过DTO、TS/Vite、rustfmt、Rust workspace和Clippy全部告警检查：**85项Rust、68项UI/API**。新增6项UI/API用例覆盖枚举/null参数、三个建议与当前语言、取消不创建资料、加载期间隐藏、目录/单文件已有授权隐藏、请求期间禁用与失败重试；新增Rust枚举采用既有序列化/生成流程，不写镜像测试。UI日期与时间线逻辑未改，纽约时区62项沿用008；扫描/并发沿用009、纯搜索规模007、时间线005，未冒充本轮重跑。

Chrome正式App+mock IPC的显式空库夹具（review.html?empty=1）复核了中文100%、英文175%/200%与返回100%的布局：动态横幅、空资料列表、设置三处建议均可读，按钮保持可见；模拟取消后仍为空库。夹具持续标识且不连接原生后端，不能证明真实选择器取消或起点。深滚动后切换页面保留旧滚动位置的导航问题已记录待修，不将其登记为体验通过。

最终release应用位于`target/release/bundle/macos/WorkTwill.app`；[010候选包](../deliverables/WorkTwill-v0.1.0-v1-candidate-010-macOS-arm64.zip)、[构建日志](validation/2026-11-20-build-v1-010.log)、[包检查](validation/2026-11-20-bundle-v1-010.log)与[源码/包体指纹](validation/2026-11-20-v1-candidate-010.json)保留。ZIP CRC、必需bundle条目及正式dist排除夹具检查通过。仍为macOS arm64、编译器ad-hoc签名，没有Developer ID/公证，不安装到Applications；旧包及历史证据保留，无Git提交或推送。

访问最终010应用，工具仍返回`Computer Use was not approved to use WorkTwill`，没有绕过限制。真实系统选择器起点/确认/取消与权限授予仍未执行，其他原生、Office、Windows、发行及真实使用关口继续保留。首次入口补齐是本轮进展，不是完整V1通过。

## 2026-11-20 迭代011：页面导航的文档滚动

复现010待修问题：英文200%空库夹具，在所有文件页向下滚动后进入设置，页面直接显示Workspace order/Local data，标题与管理位置落在视口上方。修改前通过Chrome Elements核对DOM，html带可滚动溢出标记，main计算样式overflow-x/y均visible；实际滚动所有者是整个文档，不应只设置main.scrollTop。

App新增只表示明确导航意图的本地计数，在useLayoutEffect中新页面DOM提交后调用window.scrollTo(0,0)。顶栏页面切换、工作区UUID切换、同一home视图的顶栏收藏与完整搜索入口递增；分页、业务通知刷新、页内筛选和语言切换不递增。完整搜索继续保留原关键词与范围，没有改成清空搜索后进入另一页。原生API、Rust领域状态、授权、持久数据和布局容器保持原归属。

[UI回归](validation/2026-11-20-ui-navigation-011.log)新增3项，并扩展已有完整搜索用例：核对scroll调用发生时新标题已提交、同视图收藏/不同工作区UUID回顶、刷新/分页/页内筛选/语言切换不回顶、完整搜索保留关键词且进入时回顶。jsdom只观测滚动调用时机，不能证明几何位置；Chrome同一200%夹具实际重走深滚动→设置及所有文件→顶栏收藏，标题均回到顶部，之后恢复100%。没有将浏览器夹具当成原生窗口证据。

[完整检查](validation/2026-11-20-check-v1-011.log)通过DTO、TS/Vite、rustfmt、Rust workspace与Clippy全部告警检查：**85项Rust、71项UI/API**。日期逻辑未改，纽约时区62项沿用008；扫描/并发、纯搜索、时间线规模分别沿用009/007/005，不重复记为本轮执行。

最终release应用位于`target/release/bundle/macos/WorkTwill.app`；[011候选包](../deliverables/WorkTwill-v0.1.0-v1-candidate-011-macOS-arm64.zip)、[构建日志](validation/2026-11-20-build-v1-011.log)、[包检查](validation/2026-11-20-bundle-v1-011.log)和[源码/包体指纹](validation/2026-11-20-v1-candidate-011.json)保留。仍为macOS arm64、编译器ad-hoc签名，没有Developer ID/公证，不安装到Applications；历史包与证据保留，无Git提交或推送。

本轮访问最终011原生应用仍返回`Computer Use was not approved to use WorkTwill`，没有绕过限制。其他V1关口继续保留。此次仅完成文档滚动的导航修复；原生窗口焦点/滚动、实际拖入与Office完整保存取回、Windows、发行和真实用户重复使用仍不能由本轮测试推定通过。

## 2026-11-20 迭代012：失败反馈的双语与真实空态

对照PRD F10发现业务错误只有中文，catch提前转换后无法随界面语言更新。AppError构造强制code/message/message_en，IO/SQLite和native watcher/选择器/网页元数据均提供双语；ImportItem两个导入入口保留逐项失败双语，成功项为空。不改变错误码、授权、事务或持久schema。React保留原始cause和独立存在标志，App/表单/标题获取/搜索/拖入/批量/时间线在显示时选当前语言。切语言不重发命令、不抓标题或查时间线；输入草稿与日期筛选保留。批量失败的缺失名称也在显示时翻译。见[决定0010](decisions/0010-bilingual-error-feedback.md)。

视觉复核又发现时间线失败后，月历显示0条和最近资料显示空态，容易误解为真实无记录。现改为暂不可用；成功重试后才显示实际数量与空态。普通插件/JS异常正文仍原样显示，系统控件语言随OS；未授权资源的系统名称占位另列后续修复，不据此宣称全部英文流程已通过。

[初次UI日志](validation/2026-11-20-ui-errors-012.log)保留1项测试失败：新增批量文案断言跨b节点匹配，调整为对li正文核对，没有删除失败检查或改产品行为。最终新增12项UI/API回归，并扩展已有批量/时间线测试；Rust扩展真实捕获失败、替换、无授权导入及URL拒绝的双语与序列化断言，不增加镜像构造测试。[最终统一检查](validation/2026-11-20-check-v1-final-012.log)通过DTO、TS/Vite、rustfmt、Rust workspace与Clippy全部告警检查：**85项Rust、83项UI/API**；[America/New_York同组83项](validation/2026-11-20-ui-new-york-012.log)本轮真实重跑通过。4项ignored规模基准仍不计常规执行；性能数据分别沿用009/007/005，未重跑。

浏览器访问host本轮返回不可用；通过同一CUA的Chrome原生界面完成显式模拟失败夹具复核。英文100%网页标题抓取失败与保存失败可读，URL和手改标题仍在弹窗；时间线中文错误切英文、明确重试后仍显示模拟失败、200%错误与按钮布局可读，最后恢复100%。夹具持续标识合成资料/不连接原生/模拟失败，仅作为浏览器视觉证据。组件测试中的弹窗语言切换验证渲染状态，不代表原生模态下可点击背景语言按钮。

最终release应用位于`target/release/bundle/macos/WorkTwill.app`；[012候选包](../deliverables/WorkTwill-v0.1.0-v1-candidate-012-macOS-arm64.zip)、[构建日志](validation/2026-11-20-build-v1-012.log)、[包检查](validation/2026-11-20-bundle-v1-012.log)与[源码/包体指纹](validation/2026-11-20-v1-candidate-012.json)保留。ZIP 6,489,663字节，CRC、必需bundle条目及正式dist排除夹具检查通过。仍为macOS arm64、编译器ad-hoc签名，TeamIdentifier未设，无sealed resources/Developer ID/公证，不安装到Applications；历史包和证据保留，无Git提交或推送。

本轮访问最终012应用仍返回`Computer Use was not approved to use WorkTwill`，没有绕过限制。原生选择器/IPC/Finder拖入/默认程序/完整WorkTwill U8、Word/安全替换、Windows、签名安装和真实用户重复使用等继续未完成。独立对照PRD另发现U2多选后创建并加入、快速搜索默认相关性、完整搜索返回上下文与隐藏选择、英文撤权占位等可在代码层推进的缺口；下一轮继续修复，不将当前状态标为V1完成或笼统归为原生阻塞。

## 2026-11-20 迭代013：多选创建与搜索上下文闭环

独立对照PRD U2/搜索契约，发现并修复五类当前可推进的缺口。选择栏新增明确“新建工作区并加入”，弹窗保存UUID快照并显示数量；普通新建仍为空工作区。CreateWorkspace增加可选resource_ids，Rust与Link复用同一私有关联逻辑，在一个事务创建/校验/去重/投影/事件/回执；任一失效全部回滚，成功进入返回的新UUID，取消不提交、失败保留名称。None跳过序列化，保留普通创建的既有回执payload，无schema变动。

快速搜索保留后端首选UUID并映射到分组后的默认高亮；规范化同名首选等待明确选择，方向键、aria与Enter使用同一索引。完整搜索新增返回来源入口，React保存view/UUID/mode/kind，返回原分类或收藏；已确认删除的来源工作区返回集合。关键词、类型、范围、日期/路径、排序重置加载集合时均清选择；普通列表后页版本变化重读首页也清选择，避免夹带隐藏UUID。撤权系统占位按状态在显示层翻译，真实同名中文文件保留原文。详见[决定0011](decisions/0011-selected-workspace-and-search-context.md)。

[Core工作流](validation/2026-11-20-core-workspace-013.log)新增3项：五个不同目录同名原件的UUID、去重/重放、Store重开与关系/收藏/原件字节大小mtime；撤权/非法ID/超量的创建、投影、事件、关系和回执回滚及明确重试；旧格式普通创建序列化/回执兼容。独立源码复核未发现确定后端回归，不将复核计为运行测试。前端复核追加发现排序重置首页的隐藏选择，已修并加入“加载后页再排序”用例。

[最初UI日志](validation/2026-11-20-ui-workflows-013.log)、[最初统一检查](validation/2026-11-20-check-v1-013.log)、[中间统一检查](validation/2026-11-20-check-v1-final-013.log)与[纽约中间日志](validation/2026-11-20-ui-new-york-013.log)保留：新增用例未区分原生select的combobox/option，另误用ByRole不支持的exact属性；按控件名和listbox范围修正测试查询，没有删除断言或放宽行为。最终新增14项UI回归，覆盖上述正常/取消/失败、相关性/同名、返回、隐藏选择、快照及占位。

[最终统一检查](validation/2026-11-20-check-v1-delivery-013.log)通过DTO、TS/Vite、rustfmt、Rust workspace与Clippy全部告警检查：**88项Rust、97项UI/API**；[America/New_York同组97项](validation/2026-11-20-ui-new-york-final-013.log)本轮真实重跑通过。4项ignored基准不冒充常规执行；规模数据沿用009/007/005，尚须继续补多文件大月/深分页等CLI可测项，不归为原生访问阻塞。

Chrome通过CUA原生界面，在正式App+mock IPC夹具中实际多选五份文件/网页→新建并加入→进入新UUID，显示5份关联与清空选择；在该工作区切到文件分类→搜合同→完整结果，核对中文返回入口和英文200%布局，返回原工作区后仍只显示三份文件、搜索框清空，最后恢复100%。这些是模拟后端交互与布局证据，不是原生授权/SQLite/重启闭环。测试夹具保留合成/不连接原生标识，未修改真实业务资料。

最终release应用位于`target/release/bundle/macos/WorkTwill.app`；[013候选包](../deliverables/WorkTwill-v0.1.0-v1-candidate-013-macOS-arm64.zip)、[构建日志](validation/2026-11-20-build-v1-013.log)、[包检查](validation/2026-11-20-bundle-v1-013.log)与[源码/包体指纹](validation/2026-11-20-v1-candidate-013.json)保留。ZIP 6,489,979字节，CRC、必需bundle条目及正式dist排除夹具检查通过。仍为macOS arm64编译器ad-hoc签名，无TeamIdentifier/sealed resources/Developer ID/公证，不安装到Applications；历史包及失败日志保留，无Git提交或推送。

原生访问仍沿用本轮012候选实际返回的`Computer Use was not approved to use WorkTwill`，013没有重复尝试或绕过。系统选择器/IPC/Finder拖入/原生焦点缩放/默认程序/完整WorkTwill U2与U8、Office支持矩阵、Windows、签名安装及真实用户重复使用未完成；Core重开与Chrome模拟不能提升为这些场景通过。目标保持进行中。

## 2026-11-20 迭代014：多资源分页验收（正确性通过，性能未通过）

新增ignored release入口disk_pagination_acceptance与pnpm bench:pagination，补齐原先单文件事件与225网页游标用例未覆盖的多资源、长分页链。夹具100k混合实体：98,900文件资源记录、1,000网页、100工作区，400撤权诱饵仍含投影/关系/事件；100,007活动分布到99,801资源，含两个相交工作区、同名/同时间跨页、月/日边界、纽约2026年11月25小时首日。所有样本在批准临时根内新建，不制造真实办公文件。

[首轮日志](validation/2026-11-20-pagination-014.log)与[原始报告](validation/2026-11-20-pagination-preliminary-014.json)保留：完整搜索996页/99,599项、时间线976页/97,533项、30个日历点，2,236次ID/顺序/唯一/数量/日历核对全部一致；总执行759.39秒，建库7.152秒。它使用首版夹具（有重复合成完整路径）及ID/日历oracle，且运行期间与编译/小型测试有重叠，不能当作最终独立性能通过记录。

首轮分深度30个热样本P95如下，**不达标或待独立复测，不能用正确性通过掩盖**：

| 深度offset | 完整搜索P95 ms | 时间线P95 ms |
|---:|---:|---:|
| 0 | 625.818 | 521.620 |
| 100 | 165.274 | 478.325 |
| 10,000 | 101.274 | 369.748 |
| 末页99,500 / 97,500 | 258.636 | 1076.417 |

既有SQLite热搜索P95≤60ms保持不变，没有给完整搜索静默豁免。初步源码复核发现完整页始终FTS驱动、参数CASE排序，可能无法像quick查询一样按名称索引早停；这是待查询计划/独立测量确认的优化方向，不是已实施结论。时间线尚无新增独立深页SLA，仍须如实报告其耗时与原生影响，不能声明交互性能通过。

补强版改用不同的合成文件路径；增加资源正文/关系/时间/版本字段校验，缺页与API错误也保存报告后失败；另测已有v3库补建三个派生索引的应用初始化成本（不清OS缓存）。[最终复测日志](validation/2026-11-20-pagination-final-014.log)和[最终报告](validation/2026-11-20-pagination-final-014.json)已完成：996页搜索99,599项、976页时间线97,533项、30日历点，2,239次独立核对全部一致。建库8.024秒；事件增加14,602,240字节；已有v3库补建三个派生索引483.489ms（未清OS缓存）。

最终搜索完整链P50/P95/P99为123.883/255.591/470.128ms，时间线447.183/588.169/1149.990ms。搜索首页/100/10k/末页的30热样本P95为95.422/532.566/96.581/295.950ms；时间线对应为1041.644/305.618/474.676/1004.543ms。**测试exit0只表示正确性通过；既有搜索60ms预算未通过，时间线交互性能亦不能宣布通过。**详见[决定0012](decisions/0012-multi-resource-pagination-acceptance.md)。

## 2026-11-20 迭代015：恢复忙锁与复制回滚

核查本地rusqlite实现后，用真实临时库持有外部BEGIN IMMEDIATE写锁；[旧实现复现](validation/2026-11-20-restore-lock-before-015.log)在3秒watchdog时未返回，仍持Store gate。测试释放外锁后收完线程再断言失败，没有遗留无限线程。此为确定恢复缺陷，不是原生权限阻塞。

私有copy_snapshot沿用100页/正常More的5ms节流，Busy/Locked明确database_busy，不外层重试；目标连接统一1500ms等待，未为大库复制设置任意短总时限。普通备份/incoming验证与复制/current版本与安全副本均固定源读快照，避免外写使More反复重启；所有源事务在最终target前释放，staging私有稳定。Backup正常Drop回滚未完成目标，再释放gate；只有最终复制成功才epoch+1。授权清零、回执删除与版本提升仍在staging，失败保留当前状态。见[决定0013](decisions/0013-backup-snapshots-and-busy-errors.md)。

[最终六项故障用例](validation/2026-11-20-restore-faults-final-015.log)通过：锁时返回约1618ms、gate可再查询且数据/版本/epoch不变、解除外锁明确重试；直接复制忙锁约1630ms；真实step(1)→More后Drop恢复目标原字节并可继续写；WAL外写时源快照不变；恢复当前库路径；max_page_count注入SQLITE_FULL得到storage_full、目标原字节/完整性保留、放开限制可重试。SQLITE_FULL属于复制层限制，不冒充填满OS磁盘、介质损坏或断电。

[最终完整检查](validation/2026-11-20-check-v1-final-015.log)通过DTO、TS/Vite、rustfmt、Rust workspace和Clippy全部告警检查：**94项Rust、97项UI/API**。新分页基准使ignored增为5项，常规检查不将它们计为执行；014显式运行另记。UI未改，纽约97项沿用013，不重复记为015执行。独立只读复核未发现确定修复缺陷，不计为实际测试。

本机release应用为`target/release/bundle/macos/WorkTwill.app`；[015候选包](../deliverables/WorkTwill-v0.1.0-v1-candidate-015-macOS-arm64.zip)、[构建日志](validation/2026-11-20-build-v1-015.log)、[包检查](validation/2026-11-20-bundle-v1-015.log)与[源码/包体指纹](validation/2026-11-20-v1-candidate-015.json)保留。ZIP 6,489,962字节，CRC/必需条目/正式dist排除夹具检查通过，仍为arm64编译器ad-hoc签名，无TeamIdentifier/sealed resources/Developer ID/公证，不安装到Applications；无Git提交或推送。

原生仍沿用本轮012实际访问拒绝，015未重复尝试或绕过。014最终分页复测和搜索性能处理仍可独立推进，目标保持active；原生系统选择器/IPC/Finder拖入/默认软件/完整WorkTwill U2与U8、真实Office支持矩阵、Windows、OS故障与正式安装发行、真实用户重复使用继续未完成。


## 2026-11-20 迭代016：完整搜索名称分页

014最终正确性核对通过但完整搜索超60ms预算后，先以实际Rust SQLite计划修复查询驱动和固定ORDER BY：仅高命中名称排序复用已有名称索引，其他低命中保留FTS。中间[报告](validation/2026-11-20-pagination-search-016.json)核对1,130次零错，但末页P95仍90.152ms，保留未达标证据，没有放宽预算。

名称续页改为规范化name/实体ID的tuple seek，位置在同一事务内从实际返回的第100项搜索投影取得。cursor升级协议2，保留session/权限/revision/全部条件及逻辑offset检查；旧协议、缺位置或无效位置明确stale_cursor。相关性/修改时间仍OFFSET，不将名称测量推广为三排序通过。见[决定0014](decisions/0014-complete-search-name-seek.md)。

独立复核发现NFC再小写不幂等；[修复前真实Store回归](validation/2026-11-20-unicode-seek-before-016.log)复现合法游标stale。移除位置二次规范化判等，保留原投影键；105条同名Unicode网页跨第100项的续页、全量ID顺序和无丢重已加入最终检查。初次计划断言因实际SQLite显示tuple表达式而非单name表达式失败，保留[日志](validation/2026-11-20-name-seek-plan-assertion-failed-016.log)，随后按实际范围计划校验。

[最终release日志](validation/2026-11-20-pagination-search-final-016.log)和[JSON](validation/2026-11-20-pagination-search-final-016.json)在Unicode修复后独立执行：100k实体、98,900不同合成文件路径，完整996页/99,599项，1,130次独立ID/顺序/正文/关系/版本/筛选核对，零失败；名称全链P50/P95/P99为7.011/7.448/8.685ms。首页/100/10k/末页各30热样本P95为8.747/6.996/7.333/7.167ms，均低于原60ms预算。已有v3库补建三索引189.471ms，非冷OS缓存。续页实际Rust3.50.2计划为SEARCH名称索引tuple范围、无临时排序；SQL前仍有2049有界FTS路由，不能称API完全不访问FTS。

报告明确timeline_executed=false，时间线保留014最终超标结果；不是原生IPC/键入到可见、真实办公文件或冷磁盘证明。相关性/修改时间的代表筛选仅核首页正确性，未取得其完整深页性能通过证据。

[最终统一检查](validation/2026-11-20-check-v1-final-016.log)通过：97项Rust、97项UI/API、DTO/TS/Vite/rustfmt/Clippy零告警；5项ignored基准不计为常规执行。UI行为未变，纽约97项保留013实际证据，不计为016重跑。

[016候选包](../deliverables/WorkTwill-v0.1.0-v1-candidate-016-macOS-arm64.zip)及[构建](validation/2026-11-20-build-v1-016.log)、[包检查](validation/2026-11-20-bundle-v1-016.log)、[源码/包体清单](validation/2026-11-20-v1-candidate-016.json)已保全。ZIP 6,502,666字节，CRC/必需条目/正式dist排除夹具通过。arm64编译器ad-hoc签名，无TeamIdentifier/Developer ID/公证；没有安装到Applications。原生访问仍为012实际拒绝，016未重试或绕过；UI/Office/Windows/正式发行与真实用户验收缺口保留，goal继续active。


## 2026-11-20 迭代017：时间线成本诊断与关联查询

[before日志](validation/2026-11-20-timeline-profile-before-017.log)/[JSON](validation/2026-11-20-timeline-profile-before-017.json)与[after日志](validation/2026-11-20-timeline-profile-after-017.log)/[JSON](validation/2026-11-20-timeline-profile-after-017.json)采用相同100k实体/100007事件，每深度4样本、各16独立oracle核对零错。测试专用分段执行共用正式SQL生成器；采集计划不混入计时，不称正式API或稳定P95。

确定首页约300ms主要为日历约290ms；末页还叠加约290ms事件offset跳过成本。工作区反向关系原计划SCAN，补非唯一(resource_id,workspace_id)覆盖索引后SEARCH，首页资源DTO约6.5→1.6ms；日历与深offset瓶颈保留，不能宣布时间线性能通过。有列表日期时直接生成时间范围，双时间维度/单侧和半开边界/实际范围计划已回归；无额外DTO、缓存或服务端游标状态。见[决定0015](decisions/0015-timeline-query-costs.md)。

[统一检查](validation/2026-11-20-check-v1-final-017.log)通过99项Rust、97项UI/API、DTO/TS/Vite/rustfmt/Clippy。新增旧v3索引补建（20关系/DTO/版本/完整性）及6组合日期范围计划回归。5项ignored基准仍不计为常规执行；本轮仅显式运行分段诊断，没有重新执行014千页时间线链或016名称全链。

[017候选包](../deliverables/WorkTwill-v0.1.0-v1-candidate-017-macOS-arm64.zip)已完成本机release构建；ZIP 6,502,558字节，CRC/必需条目/正式dist排除视觉夹具通过。见[构建](validation/2026-11-20-build-v1-017.log)、[包检查](validation/2026-11-20-bundle-v1-017.log)及[指纹清单](validation/2026-11-20-v1-candidate-017.json)。仍为arm64编译器ad-hoc签名，无TeamIdentifier/Developer ID/公证；未安装到Applications。原生验收继续沿用012实际访问拒绝，017未重试或绕过。


## 2026-11-20 迭代018：版本一致的有界月历复用

最多4项、每Store独占的LRU只保存整月日期计数。每次从同一SQLite读事务取得权限/revision快照后，再精确匹配日界/filter/basis/workspace；锁不包SQL。事件和资源正文仍每次查询。读版本、恢复、迟到旧版本和正式写路径审计见[决定0016](decisions/0016-versioned-calendar-cache.md)。

[定向回归](validation/2026-11-20-calendar-cache-targeted-018.log)11项通过，新增key隔离/分页复用/容量淘汰/正文独立读取，以及转移/排除/恢复/活动变更失效。直接SQL测试helper和单文件100k→500k基准每档seed遵循事务bump；单文件规模基准本轮没跑，不能混为新的执行证据。

[最终完整release日志](validation/2026-11-20-pagination-final-018.log)与[JSON](validation/2026-11-20-pagination-final-018.json)：996搜索页99,599项、976时间线页97,533项、30日历点，2,239独立核对、零失败；名称搜索全链P50/P95/P99为2.110/2.368/5.050ms，四深度热P95均2.05–2.48ms，仍低于60ms既有预算。

正式Store::timeline首次日历未命中**292.500ms**（单次、不称冷OS磁盘或P95）。1099次日历命中；最多4项。时间线完整链P50/P95/P99为153.523/287.518/436.390ms；首页/100/10k/末页各30热样本P95为2.617/2.674/32.995/**301.891ms**。缓存解决重复整月计算；首次月份及深offset未解决，不声明时间线整体性能通过。原生日历/文件外部变化到窗口更新及真实可见时延仍未验收。

[统一检查](validation/2026-11-20-check-v1-final-018.log)通过101项Rust、97项UI/API、DTO/TS/Vite/rustfmt/Clippy。5项ignored基准未计为常规执行；多资源完整分页显式运行如上，其他旧性能证据保留原轮次。UI未变，纽约97项仍保留013实际结果。

[018候选包](../deliverables/WorkTwill-v0.1.0-v1-candidate-018-macOS-arm64.zip)已完成本机release构建；ZIP 6,507,647字节，CRC/必需条目/正式dist排除视觉夹具通过。见[构建](validation/2026-11-20-build-v1-018.log)、[包检查](validation/2026-11-20-bundle-v1-018.log)及[指纹清单](validation/2026-11-20-v1-candidate-018.json)。仍为arm64编译器ad-hoc签名，无TeamIdentifier/Developer ID/公证；未安装到Applications。原生验收继续沿用012实际访问拒绝，018未重试或绕过。下一轮继续时间线末项游标和默认相关性查询性能，目标保持active。


## 2026-11-20 迭代019：时间线版本化末项游标

时间线追加页不再OFFSET，改为有效时间/ID双键继续；cursor绑定完整条件/session/权限/revision和逻辑next_offset。无cursor的旧offset、无效位置和旧条件明确stale_cursor，UI原样回传且仅append失效重读一次首页。observed增加双DESC派生索引，新旧v3/恢复迁移统一补建。详见[决定0017](decisions/0017-timeline-seek-cursor.md)。

中间[tuple-only报告](validation/2026-11-20-pagination-tuple-only-019.json)已正确但末页53.275ms；仅加标量上界的[竞争范围报告](validation/2026-11-20-pagination-competing-upper-019.json)仍54.006ms。最终续页以原日期上界为残余过滤，让索引采用游标时间上界，完整日期与授权过滤保留。不是调高预算或减少结果。

[最终release日志](validation/2026-11-20-pagination-final-019.log)和[JSON](validation/2026-11-20-pagination-final-019.json)：996搜索页99,599项、976时间线页97,533项、30日历点，2,239独立核对全部一致。所有深页请求来自真实cursor链，未仅修改offset构造。时间线全链P50/P95/P99为2.487/2.774/4.223ms，首页/100/10k/末页30热样本P95为2.472/2.699/2.604/1.306ms；首次日历未命中288.760ms为单样本、非冷OS磁盘。名称搜索全链P95为2.229ms，四深度均低于60ms。相关性/修改时间仅核首页正确性，仍未取得其深页性能通过证据。

[统一检查](validation/2026-11-20-check-v1-final-019.log)通过104项Rust、100项UI/API及DTO/TS/Vite/rustfmt/Clippy；[纽约时区](validation/2026-11-20-ui-new-york-019.log)100项实际重跑通过。新增16种cursor拒绝、mutation/reopen/持cursor恢复、两种时间225项混合/同时间戳跨3页、伪造合法位置仍不能绕过授权和选日。UI覆盖append失效一次恢复、首页失败不循环，以及旧append晚到后仍用新筛选cursor。最初测试参数TS不支持exact、诊断器多余引用的Clippy失败日志保留；正式检查已修复后通过。

共享SQL诊断仍强制计算日历且不验证cursor，不作为正式API SLA；最终正式全链数字来自Store::timeline。原生IPC/可见时延/文件外部变化到窗口、真实Office及发行仍未完成。F09复核确认独立文件或整个根外部移位的原生locator尚未生产实现，不能仅归为GUI未验；WPS原位保存有限实测和未知替换人工修复边界保持。

[019候选包](../deliverables/WorkTwill-v0.1.0-v1-candidate-019-macOS-arm64.zip)完成本机release构建，ZIP 6,521,234字节，CRC/必需条目/正式dist排除视觉夹具通过。见[构建](validation/2026-11-20-build-v1-019.log)、[包检查](validation/2026-11-20-bundle-v1-019.log)、[源码/包体清单](validation/2026-11-20-v1-candidate-019.json)。仍为arm64编译器ad-hoc签名，无TeamIdentifier/Developer ID/公证；未安装到Applications，无Git提交或推送。原生访问仍沿用012实际拒绝，019未重试或绕过。

新增[macOS locator研究](research/MACOS_LOCATOR_BOUNDARIES.md)将确定生产缺口与后续安全实现分开：Bookmark只能定位候选；单文件同物理对象恢复可独立增量；root定位需同时处理物理锚点、有效授权、扫描和监听，不可用一次路径赋值冒充完成。它是研究交付，未实现或测试新定位能力。目标继续active，后续先完成默认相关性/修改时间的性能证据，再推进可独立的剩余实现与原生验收。


## 2026-11-20 迭代020：默认相关性有序分桶与末项游标

完整搜索协议3为相关性新增rank/name/id末项位置，复用quick四档互斥规则与名称索引；名称续读保持，modified仍OFFSET。每档授权、工作区、类型、日期、字面路径筛选在LIMIT前，FTS路由探测最多2049条，不能因桶计划无虚表就声称整个API不访问FTS。见[决定0018](decisions/0018-relevance-rank-seek.md)。

[前置首页profile](validation/2026-11-20-complete-search-profile-before-020.json)中相关性all/file/date P95为91.127/101.247/289.892ms；[最终9案例profile](validation/2026-11-20-complete-search-profile-final-020.json)为9×2排序×31次，共558独立核对零错，相关性P95为0.559–8.978ms。modified的all/file/date/英文路径/中文路径仍88.773/89.731/86.070/109.238/132.353ms，**未通过60ms预算**。这组仅测首页，不能冒充深页证据。

[相关性完整链日志](validation/2026-11-20-pagination-relevance-final-020.log)及[报告](validation/2026-11-20-pagination-relevance-final-020.json)实际完成996搜索页99,599项、976时间线页97,533项、30日历点，2,239核对零错。相关性全链P50/P95/P99为4.744/7.098/7.531ms；首页/100/10k/末页30热样本P95为2.556/2.258/2.980/18.614ms，均低于60ms。主链主要为前缀档，另用3k混合原始oracle覆盖四档跨界及8组筛选全链。时间线本次全链P95为2.665ms，首次月历未命中293.498ms为单样本，非冷OS磁盘或P95。

[统一检查](validation/2026-11-20-check-v1-final-020.log)通过107 Rust、100 UI/API及DTO/TS/Vite/rustfmt/Clippy。前次新测试类型复杂度Clippy失败日志保留，已改具名原始记录后通过。UI运行时未改，仅更新mock游标；纽约100项沿用019实际结果，不声称020重新执行。NUL[修复前红测](validation/2026-11-20-nul-query-before-020.log)证明此前未拒绝输入，共享校验已修；高命中漏页风险来自源码/SQLite语义核查，未写成实际复现。

本轮仍未执行原生窗口操作，沿用012的CUA访问拒绝。F09 locator生产实现、Office安全保存、Windows、安装签名、首次月份和真实重复使用关口保持未完成。目标继续active，下一轮处理修改时间排序与可独立实现的定位能力。

[020候选包](../deliverables/WorkTwill-v0.1.0-v1-candidate-020-macOS-arm64.zip)完成本机release构建，ZIP 6,537,882字节，CRC/必需条目/正式dist排除视觉夹具通过。见[构建](validation/2026-11-20-build-v1-020.log)、[包检查](validation/2026-11-20-bundle-v1-020.log)、[源码/包体清单](validation/2026-11-20-v1-candidate-020.json)。仍为arm64编译器ad-hoc签名，无TeamIdentifier/Developer ID/公证；未安装到Applications，无Git提交或推送。


## 2026-11-20 迭代021：修改时间派生排序与三段末项续读

schema v4仅在搜索投影新增nullable modified_at及全局/kind时间索引，资源表仍为事实源。Catalog、导入、重新指向和网页保存同事务使用同一值；工作区NULL，资源0/负时间保留。project仅mtime改变时只UPDATE投影一行，不重写FTS；全部不变时保持零写入。v3→v4的ALTER/回填/索引/版本推进同事务，旧库不在每次打开重复回填。详见[决定0019](decisions/0019-modified-projection-and-seek.md)。

SearchCursor协议4增加modified_position，三种位置槽互斥，位置从同读取事务实际第100项的投影取得。高命中修改排序拆同时间name/id余项→较早时间→NULL工作区，NULL位置只在NULL段续读；每段完整过滤后最多补齐101。低命中仍FTS复合续读，全部排序实际OFFSET为0。阶段计划回归确认Equal/Null续页名称ID范围、Earlier时间范围且无临时排序，不仅检查首页。

[最终完整链日志](validation/2026-11-20-pagination-modified-final-021.log)及[JSON](validation/2026-11-20-pagination-modified-final-021.json)：100k混合库996搜索页99,599项、976时间线页97,533项、30日历点，**2,240核对零错**。modified全链P50/P95/P99为2.527/2.785/2.965ms，首页/100/10k/末页30热样本P95为**2.862/2.761/2.804/0.782ms**，均低于60ms预算。时间线全链P95为2.837ms；首次月份未命中305.707ms为单样本，非冷OS磁盘或P95。先前[迁移profile补强前报告](validation/2026-11-20-pagination-modified-pre-migration-profile-021.json)保留，正确性也通过，不混为最终数据。

[9案例首页profile](validation/2026-11-20-complete-search-profile-final-021.json)实际558核对零错，modified案例0–8热P95为2.698/16.101/2.517/2.797/7.480/0.581/3.038/2.631/0.774ms，均低于60ms；020相同all/file/date/英文路径/中文路径为88.773/89.731/86.070/109.238/132.353ms。相关性本次首页0.613–9.195ms，其完整深链沿用020；名称完整链沿用019。首页profile不代表所有筛选的深页或原生可见延迟。

100k结构v3→v4样本打开迁移491.751ms；该规模样本明确通过备份并移除v4新列/索引构造结构v3，真实历史schema链另由小型迁移回归证明。当前v4库补建三个既有派生索引538.845ms，与新增列迁移分开。均未清OS缓存。100k建事件前/后checkpoint库体291,065,856/308,920,320字节；019为279,539,712/297,390,080字节，包含新列和两个时间索引的存储代价，非单独索引大小测量。

[10k实际合成文件日志](validation/2026-11-20-scan-10k-final-021.log)和[JSON](validation/2026-11-20-scan-10k-final-021.json)：初扫3.000秒、首次严格扫描中可查观察约28ms；不变重扫0.559秒、一次mtime修改重扫0.558秒，对应并发quick P95为8.981/2.831/2.799ms。10k UUID/路径/metadata、10k首次发现/1次修改、全部派生mtime与事实一致，目标工作区/收藏/字节保持。库体33,292,288→33,349,632字节。不是100k实扫重跑、办公资料、原生窗口或空闲资源证明；不能把不同轮次时延差异全部归因于新索引。

[统一检查](validation/2026-11-20-check-v1-final-021.log)通过**115 Rust、100 UI/API**及DTO/TS/Vite/rustfmt/Clippy。新增真实无新列v3回填/迁移失败回滚重试/重复重开/旧备份恢复，零负时间/多页NULL工作区/8筛选组合与三槽拒绝；生产写路径从初扫、mtime-only、改名/移动、工作区/网页、单文件、人工合并一直到重建/恢复均核对投影一致性。5项ignored不计为常规执行，本轮显式规模执行如上。UI运行时未改，仅更新mock协议，纽约100项仍为019实际记录。

原生窗口/选择器/IPC/Finder拖入/默认软件/U2/U8仍未执行，沿用012的CUA访问拒绝；021未重复尝试或绕过。F09单文件/根locator生产实现、Word安全保存、多卷/Windows、签名安装和真实重复使用仍未完成。目标继续active，不能把搜索预算通过写成整个V1已交付。

[021候选包](../deliverables/WorkTwill-v0.1.0-v1-candidate-021-macOS-arm64.zip)完成release构建，ZIP 6,537,883字节，CRC/必需条目/正式dist排除视觉夹具通过。见[构建](validation/2026-11-20-build-v1-021.log)、[包检查](validation/2026-11-20-bundle-v1-021.log)、[源码/制品指纹](validation/2026-11-20-v1-candidate-021.json)。arm64编译器ad-hoc签名，无TeamIdentifier/Developer ID/公证，未安装到Applications，无Git提交或推送。

记录更正：020清单的archive摘要误沿用019标识；020实际ZIP及artifact_sha256已正确记录。本轮只更正该摘要为020的路径/字节/hash并保留更正说明，没有替换020包。


## 2026-11-20 迭代022：macOS 独立授权文件的原生定位

schema v5新增私有file_locators，Core直接使用已锁定objc2 0.6.5/Foundation 0.3.2，无版本升级。显式单文件导入/重新指向在锁外创建普通Bookmark；原路径失效后只对当前活跃file grant尝试一次原生候选解析。候选必须匹配原物理身份；原ingest事务复核原id/identity/location/BLOB/权限/revision、当前grant和候选metadata，才更新路径/搜索/metadata并保留UUID与关系。不为全量目录索引捕获，不写源文件/自定义追踪数据。见[决定0020](decisions/0020-single-file-native-locator.md)。

[统一检查](validation/2026-11-20-check-v1-final-022.log)最终**133 Rust、100 UI/API**通过，DTO/TS/Vite/rustfmt/Clippy通过，5项ignored不计常规执行。4项真实Foundation API含改名/同卷移动、新测试进程解析、删后同名身份负例、无效/非本地URL拒绝；12项真实Bookmark领域回归含两个工作区/收藏/原件字节及mtime、Store重开、scan/resolve、邻居不获权、副本/替换不继承、撤权/恢复、提交间mutation/relink/restore/原BLOB变化、候选身份硬约束、relink/merge、旧grant补建、坏定位不阻塞无关根、过时错误丢弃、当前创建失败仍报告。旧grant只能在原位置实际身份正确时补建。另测v4建表故障回滚可重试、schema5备份缺表/必要列拒绝且当前库无损。

独立审查发现并修复了三项实际源码缺口：坏Bookmark曾提前中止所有root；恢复结构检查遗漏新表；失败重核前已收集的旧错误会在relink/撤权后继续报告。新回归覆盖其行为，不将静态发现伪装成修复前红测。refresh现在使用恢复的新路径；状态仅实际变化才bump，定位错误先完成无关roots再明确返回，DB/锁错误仍立即停止。原生open/reveal正常返回或失败均发一次catalog-changed，因为定位可能已写入而系统随后失败；系统拒绝仍不记opened。此IPC通知仅源码核对/编译，未实际原生操作。

[本轮release规模日志](validation/2026-11-20-pagination-modified-final-022.log)与[JSON](validation/2026-11-20-pagination-modified-final-022.json)：schema5的100k混合库996搜索页99,599项、976时间线页97,533项、30日历点，**2,240核对零错**。修改排序全链P50/P95/P99为2.536/2.788/3.299ms，首页/100/10k/末页热P95为2.695/2.766/2.615/0.734ms，均小于60ms。时间线全链P95为2.797ms，首次日历未命中294.776ms为单样本、非冷OS磁盘。结构v3→当前v5迁移584.704ms，规模fixture明确由新库备份移除v4列/索引和v5表构造；真实历史链由小型回归覆盖。9案例首页/实际10k扫描保留021，相关性完整链保留020、名称019，不冒称本轮重测所有排序和负载。

UI运行时未改，100项回归本轮执行，纽约100仍为019证据。API新测试进程仅证明Bookmark跨测试进程解析，Store重开不等于正式WorkTwill窗口退出重启。Foundation WithoutUI/WithoutMounting不构成严格耗时上限；普通Bookmark不是security-scoped权限或Document ID证据。

原生窗口/选择器/IPC/Finder拖入/默认程序/U2/U8沿用012实际CUA访问拒绝，本轮未重试或绕过。整个root及仅目录授权的重要引用外部移位的生产locator、Document ID/身份变化安全保存、Word/跨卷/云目录/Windows、签名安装、OS故障及真实重复使用仍未完成。F01–F14/U1–U8范围不缩减，目标继续active。

[022候选包](../deliverables/WorkTwill-v0.1.0-v1-candidate-022-macOS-arm64.zip)完成release构建，ZIP 6,548,240字节，CRC/必需条目/正式dist排除视觉夹具通过。见[构建](validation/2026-11-20-build-v1-022.log)、[包检查](validation/2026-11-20-bundle-v1-022.log)、[源码/制品指纹](validation/2026-11-20-v1-candidate-022.json)。arm64编译器ad-hoc签名，无TeamIdentifier/Developer ID/公证，未安装到Applications，无Git提交或推送。


## 2026-11-20 迭代023：独立文件授权摘要与状态可见性

首页此前只用activeRoots.length显示数量，仅有file_grants时误报“已管理0个位置”。现分别显示目录数与单独授权文件数，覆盖仅文件/仅目录/混合/空/扫描、暂停/撤权及失联仍有授权的事实；中英文支持英文单复数。设置单文件卡片保留路径，用既有短文案区分“原件暂不可用/Original unavailable”“身份待确认/Identity unconfirmed”，正常文件不增加状态说明。没有新增数据源、接口、持久状态或额外授权流程，见[决定0021](decisions/0021-authorized-scope-summary.md)。

[统一检查](validation/2026-11-20-check-v1-final-023.log)通过**133 Rust、107 UI/API**及DTO/TS/Vite/rustfmt/Clippy。[纽约时区](validation/2026-11-20-ui-new-york-023.log)107项本轮实际重跑通过。新增六组双语摘要组合与一项单文件状态回归，核对切语言不重做授权或mutate。5项ignored未计常规执行，后端查询/迁移/原生逻辑未改，规模数字保留022/021/020/019对应实际轮次，没有为UI文字修改重复冒称新规模测试。

[浏览器观察记录](validation/2026-11-20-browser-scope-summary-023.json)：CUA实际通过Google Chrome原生AX/截图读取正式App+mock IPC夹具scope=files-only，合成三个独立授权文件（正常超长英文名/单段路径、失联、身份待确认）。中文100%“已管理3个单独授权文件”、英文200%“Managing 3 individually authorized files”摘要可读且无0目录误报；英文200%设置长路径在卡片内换行，状态与Revoke可见；最后恢复中文100%。截图在工具结果中实际观察，未将PNG保存到仓库。页面持续“合成资料、不连接原生后端”标识，不连接SQLite/原生服务；浏览器host不可用时改用已可访问的Chrome原生界面，不是绕过WorkTwill应用访问拒绝。

新增[root完整定位实施研究](research/ROOT_LOCATOR_IMPLEMENTATION_PLAN.md)，列明物理锚点/有效授权、历史别名仅候选、暂停只复验既有资源、重选同身份复用UUID、版本/恢复/监听/扫描收尾与必要负例。它是待实施研究，不是新增root能力。当前生产仍为022单文件增量，F09 root/Document ID/安全替换缺口继续存在。

原生窗口/选择器/IPC/Finder拖入/默认程序/U2/U8沿用012实际拒绝，本轮未再次尝试WorkTwill或绕过。Word/跨卷/云目录/Windows、签名安装、OS故障与真实重复使用仍未完成，范围不缩减，目标继续active。

[023候选包](../deliverables/WorkTwill-v0.1.0-v1-candidate-023-macOS-arm64.zip)完成release构建，ZIP 6,548,243字节，CRC/必需条目/正式dist排除视觉夹具通过。见[构建](validation/2026-11-20-build-v1-023.log)、[包检查](validation/2026-11-20-bundle-v1-023.log)、[源码/制品指纹](validation/2026-11-20-v1-candidate-023.json)。arm64编译器ad-hoc签名，无TeamIdentifier/Developer ID/公证，未安装到Applications，无Git提交或推送。

## 2026-11-20 迭代024：整个管理目录的同物理身份定位

新授权目录在schema v6保存私有root_locators及root_aliases。active授权意图与valid位置有效性分开；新授权在锁外捕获普通Bookmark、提交前复核identity。同身份明确重选复用root UUID和相对排除，不同身份清旧aliases，路径另属其他root明确冲突。legacy旧目录不在migration或启动中追认历史身份，设置提示明确重选。Store::open仅metadata核验；范围变化与generation/p/r同事务，查询、日历和游标失效。详见[决定0022](decisions/0022-root-native-locator.md)。

Root恢复锁外Foundation、短事务CAS核对root/path/identity/BLOB/valid/generation/active/p/r，aliases只按路径组件构造候选；原ingest再核文件原identity和当前scope/exclusion，不新增file grant、不按前缀批改。不暂停时走正式扫描，暂停时稳定UUID seek核对已有映射。Root/File两种guard各自守authority，root BLOB不写入file_locators。有效scope接入SQL可见性、导入/relink归属、扫描setup/valid/commit/finalize及监听；旧批次失效有正确收尾，restore epoch阻止代次ABA。后台空源/暂停根有30→300秒退避入口，显式重新扫描/重选和恢复会重试监听；这部分原生watch接线仅编译/源码核对，不冒充系统事件实测。

[最终统一检查](validation/2026-11-20-check-v1-final-024.log)通过 **161 Rust、116 UI/API** 及DTO/TS/Vite/rustfmt/Clippy；[纽约时区](validation/2026-11-20-ui-new-york-024.log) **116/116** 本轮实际重跑。常规检查有6项ignored，只有下述两种本轮显式执行的规模验收可计规模证据。新增28项Rust回归：3项目录实际API（总7项macOS Bookmark API），17项目录领域，2项guard，6项扫描/权限并集；另有9项UI状态覆盖。

覆盖实际目录改名/同卷移动/父目录改名，Store重开、连续移动、UUID/工作区/收藏/字节/mtime；暂停205引用跨三批且不提前收录新增；同名替换、复制、符号链接、排除、撤权、根冲突、同/异身份明确重授；真实v5→v6迁移回滚重试及schema6坏表/列备份不覆盖；启动不枚举/正常版本稳定、失效p/r/gen各+1及旧cursor不可复用。捕获guard后撤权/暂停/排除/恢复包含/regrant/relink/restore都拒旧提交；入口漏传expected_identity也不能接入替代文件。旧root移动/替换时，即使文件硬链接身份相同也拒旧batch；旧收尾不能覆盖新扫描；另一失效root不保护missing。

独立审查发现并修复两个具体消费者遗漏：root_unavailable原会从混合file/root授权穿透，阻塞无关root扫描；legacy仍允许扫描却被前端漏报索引。另修复scope并集状态：独立file grant仍有效时，root排除扫描不能把文件误标missing，删除后仍由单文件复验正确标missing。回归核持久status、解析路径和原件字节，不仅断言可见；这些源码发现不伪装成修复前红测。

实际API边界：首轮目录领域8/9中，旧路径被新目录占用的用例因root_unavailable失败；它错误假定Bookmark必定找到移走原目录。实际Foundation可能返回替代目录，生产identity校验正确拒绝。保留防误连检查，测试分别核拒绝、零隐式授权/误继承，再明确重选原目录恢复同root UUID。见[观察记录](validation/2026-11-20-root-bookmark-boundary-024.json)。另[初次统一检查](validation/2026-11-20-check-v1-before-024.log)在测试文件追加的中间态遇rustfmt差异；[启动定向初测](validation/2026-11-20-root-domain-024.log)15/17，两个新夹具用了不允许的空查询，改成真实101项匹配后[17/17](validation/2026-11-20-root-domain-final-024.log)，没有修改产品输入语义或降低断言。

### 10k真实合成文件目录恢复

[最终日志](validation/2026-11-20-root-locator-scale-024.log)及[JSON](validation/2026-11-20-root-locator-scale-024.json)为release、磁盘WAL/FULL、100子目录/10,000真实小文件；本轮第二次独立fixture，无同时统一检查任务。全部原UUID、路径/identity/mtime/size、投影/FTS、发现活动、零单文件授权与locator、两次aliases和再次重开持久化逐项核对；暂停新增负例不收录，恢复扫描后仅新增1UUID，无伪修改事件。只对7个样本核原件字节，不宣称10k正文全部检查。

| 阶段 | 操作秒 | 并发quick样本 | P50 / P95 / P99 ms |
|---|---:|---:|---:|
| 初扫 | 2.994 | 305 | 1.136 / 10.460 / 13.387 |
| 暂停目录移动，恢复全部已知映射 | 27.838 | 2,576 | 2.604 / 4.602 / 12.789 |
| 重开后二次移动，7个引用按需恢复 | 0.104 | 5 | 17.991 / 18.542 / 18.542 |
| 恢复扫描，收录新增并归一全部路径 | 3.339 | 258 | 1.638 / 13.756 / 25.457 |

暂停恢复确有约28秒成本，不能因为“不枚举新增”就称I/O很低；query期间允许实际空/部分结果，全部无查询错误。短按需阶段只有5计时样本，不能据其P95推断稳定尾延迟；这是Core/本地文件API，不是原生可见时延。第一次同规模运行与统一检查部分重叠，保留[负载日志](validation/2026-11-20-root-locator-scale-concurrent-024.log)/[JSON](validation/2026-11-20-root-locator-scale-concurrent-024.json)：初扫3.427秒、暂停恢复31.944秒；不与无额外检查任务的最终数据混称同一负载。

### 100k混合分页复测

[日志](validation/2026-11-20-pagination-modified-final-024.log)/[JSON](validation/2026-11-20-pagination-modified-final-024.json)：schema6、100k实体，996搜索页99,599项与976时间线页97,533项、30日历点，**2,240核对零错**。修改排序全链P50/P95/P99 **2.623/2.811/2.969ms**；首页/100/10k/末页热P95 **2.595/2.632/2.617/0.707ms**，均小于原60ms预算。时间线全链P95 **2.752ms**，首次月历未命中 **277.434ms** 单样本、非OS冷盘；不能宣称首次月历时延已全部优化或原生交互SLA通过。结构v3→v6迁移585.466ms；该规模fixture显式移除v4列/索引及v5/v6定位表，真实历史链另有小型回归。相关性020、名称019和更大quick规模仍保留原轮次，不称全部本轮重测。

设置Root状态优先撤权、定位状态、暂停、扫描结果，legacy扫描仍有首页/搜索索引提示；DTO不含原始定位数据。新增视觉夹具scope=root-locations覆盖长路径、legacy/不可用暂停/撤权。[本轮CUA记录](validation/2026-11-20-root-location-visual-024.json)：Chrome报cgWindowNotFound，browser host不可用，未观察到页面，因此024新增状态截图/200%检查**未执行**；023的实际视觉证据仍只属于023。原生WorkTwill此前访问拒绝仍未解除，本轮未绕过或重试原应用。真实选择器/IPC/Finder拖入/默认程序/U2/U8、Word/身份变化安全保存、多卷/云盘/Windows、签名安装、OS故障和真实重复使用仍未完成。F01–F14/U1–U8不缩减，目标保持active。

[024候选包](../deliverables/WorkTwill-v0.1.0-v1-candidate-024-macOS-arm64.zip)完成release构建，ZIP **6,577,680字节**，CRC/必需bundle条目/归档内二进制一致/可执行位/正式dist排除夹具通过。见[构建](validation/2026-11-20-build-v1-024.log)、[包检查](validation/2026-11-20-bundle-v1-024.log)、[源码/制品指纹](validation/2026-11-20-v1-candidate-024.json)。arm64编译器ad-hoc/linker-signed；无TeamIdentifier/Developer ID/公证，Info.plist未绑定、资源未sealed。未安装到Applications，无Git提交或推送，023历史ZIP保留。可运行候选与完整V1通过仍是不同结论。

## 迭代025：历史来源、修复影响确认与撤权连续性

本轮先完成024后的独立审查与缺陷修复。F01–F14/U1–U8保持，目标仍active；本节不把部分通过改称完整V1。实现见[决定0023](decisions/0023-history-origin-and-relink-review.md)。

[真实红测](validation/2026-11-20-u8-recovery-red-025.log)发现两项缺陷：同路径替换被commit标为identity_unconfirmed后又被finalize覆盖missing；原记录重新指向不同对象后，旧opened活动的ID/时间未变但名称变为新文件。修复保留具体身份问题；四个生产活动入口保存观察名称，React读取非空历史名，打开仍访问当前引用。

只保存名称会引入授权问题：A撤权后复用UUID重新指向已授权B，不能借B的权限暴露旧A名称。schema7私有三表保存观察UUID/identity/root scope及relink原/合并UUID与新旧identity；合并只改变规范资源引用，不重写这些观察字段。v6迁移只绑定迁移时权限基准、旧观察UUID为NULL；原始来源不可重建，未知identity历史隐藏。

全量既有安全回归实际挡住一次新缺陷，保留[失败日志](validation/2026-11-20-check-v1-history-scope-red-025.log)：A从旧root移到新root并撤新root后，旧历史scope不能覆盖已确认移出的事实。读取先服从仍存在的观察identity记录的当前权限；跨身份修复前再持久保存该identity最后已知位置对旧scope的资格，包含排除规则，避免identity退休后再复活。新增两步移动/撤权/匿名修复/恢复和同根排除链，原观察字段仍不改。NULL夹具直接写库也在同事务递增revision，才能正确作废已有月历快照；修正前的[夹具缓存失败](validation/2026-11-20-check-v1-null-fixture-cache-red-025.log)保留，不通过取消产品缓存掩盖测试问题。

原生重新指向改为Rust影响快照：确认源工作区/收藏，捕获所选原件身份；目标已收纳时列双方工作区、收藏与所选路径，提交前核对相关事实和权限版本。无关打开/扫描不废确认，影响关系变化或重复提交拒绝。双语字符串和真实Store→review→文案接线测试通过，不代表系统长对话框实际完整可读。

### 最终统一与专项检查

[最终统一检查](validation/2026-11-20-check-v1-final-025.log) **186 Rust /120 UI/API** 全通过，包括DTO漂移、TS/Vite、rustfmt和Clippy warnings-as-errors；[纽约时区](validation/2026-11-20-ui-new-york-025.log) **120/120** 实际重跑。6项ignored不计常规执行，下述规模项目另显式执行。

本轮新增25项Rust：2项真实替换/旧活动回归、8项影响审阅和过时/重放保护、7项历史授权、5项schema7迁移/恢复/来源、3项确认文案。历史测试覆盖不同身份、合并后删除目标、根移动/换锚点、排除、恢复重授权、未知identity、当前B匿名占位，直接核持久来源且保留旧安全断言。迁移测试真实手建v6，3种新表冲突原子回滚并重试；**3缺表+13缺列共16种**损坏schema7备份不能覆盖当前库。真实合并后重开、备份恢复仍保留原UUID/identity/root来源，不进入DTO。早期[5项迁移检查](validation/2026-11-20-history-migration-final-025.log)在退休资格列加入前完成，最终结构以全量日志为准；编译诊断遗漏和应用数据自排除夹具问题的失败日志均保留。

时间线诊断SQL和匿名资源映射与正式读取一致。100k分页夹具仍为旧空名兼容负载，显式绑定迁移基准；文件增加明确synthetic identity，不能用NULL未知身份充当已验证来源。oracle、排序和原60ms搜索预算未放宽；该合成库不是10万真实文件或历史来源采集实测。

### schema7规模与成本

[100k日志](validation/2026-11-20-pagination-modified-final-025.log)/[JSON](validation/2026-11-20-pagination-modified-final-025.json)：996搜索页99,599项、976时间线页97,533项、30日历点，**2,240次独立核对零错**。修改排序全链P95 **4.058ms**，首页/100/10k/末页热P95 **3.211/3.262/3.217/0.977ms**；时间线全链P95 **7.813ms**。首次月份未命中 **623.512ms**（单样本、非OS冷盘），四次强制不缓存的分段月历约576–1,736ms（包含一次明显慢样本），不与正常缓存API时延混称，不能宣称首次月历预算通过。结构v3→v7迁移约12.638秒；夹具显式移除后续结构，小型真实历史链另有回归。10万实体事件阶段数据库增加51,625,984字节，不等于长期体积验收。

10k真实小文件、100子目录、WAL/FULL，独立两次运行均核全部UUID/路径/metadata及发现/修改计数、目标工作区/收藏/原件字节。首次[日志](validation/2026-11-20-scan-final-025.log)/[JSON](validation/2026-11-20-scan-final-025.json)初扫 **11.374秒**，并发quick P95/P99 **5.453/69.332ms**；不变/一次修改重扫 **0.611/0.589秒**。因初扫明显慢而进行独立[复测](validation/2026-11-20-scan-repeat-025.log)/[JSON](validation/2026-11-20-scan-repeat-025.json)：初扫 **4.953秒**，并发quick P95/P99 **5.989/27.471ms**；重扫 **0.589/0.625秒**，P95 **3.086/2.991ms**。两次均在严格未完成初扫期间约24ms观察到可搜索结果。两次没有源码变化，不能只选择较快值作稳定保证，也不能据此分离机器抖动与新来源写入的成本；旧轮3秒结果保留其轮次。schema7实际扫描库约38.9MB。

### 原生研究与剩余验收

[Document ID只读研究](research/MACOS_DOCUMENT_ID_READONLY_RESEARCH.md)/[探针与结果](validation/2026-11-20-macos-document-id-readonly.json)：本机APFS卷能力有效，但3个新样本×2进程的Foundation值均nil、底层值0且未设置UF_TRACKED；读取前后可见stat字段未变。Apple公开HFS测试展示同路径新建可继承旧Document ID，不能把HFS行为冒充APFS保证，也不能把非零相等直接当自动合并证据。本轮不写源metadata、不实施Document ID自动继承，研究不算U8安全保存通过。

025尝试访问WPS仍返回cgWindowNotFound，没有新的实际Office保存结果。WorkTwill此前明确CUA访问拒绝未解除；未绕过。024新增状态视觉检查也仍未执行，023截图不升格为本轮视觉证据。原生选择器/IPC/Finder拖入/默认程序/U2/U8、长确认内容视觉、Word身份变化保存、多卷/云盘/Windows、安装签名、公证、崩溃升级与真实重复使用仍待完成。

[025候选包](../deliverables/WorkTwill-v0.1.0-v1-candidate-025-macOS-arm64.zip)已完成[release构建](validation/2026-11-20-build-v1-025.log)，ZIP **6,610,674字节**，SHA256 `70351cd970e3c7023c70e3145b7cd8e079946055b1772e75eb759468cc1e7940`。[包检查](validation/2026-11-20-bundle-v1-025.log)通过CRC、必要bundle条目、归档二进制与当前app一致、可执行位及正式dist排除视觉夹具，024 ZIP指纹保持。源码/证据/制品指纹见[025清单](validation/2026-11-20-v1-candidate-025.json)。签名仍为arm64编译器ad-hoc/linker-signed，无Developer ID/TeamIdentifier/公证/密封资源；未安装到Applications，没有Git提交或push。候选包不等于完整V1验收完成。


## 2026-11-20 迭代026：工作台默认首页、历史消费者与打开身份

V1仍为PRD F01–F14/U1–U8，没有缩减为浏览器演示或只做动态页。本轮正式源码、制品和证据以[026指纹清单](validation/2026-11-20-v1-candidate-026.json)为准。025包和日志保留，当前release App已更新为026。

### 实施及失败复现

- 共用历史授权SQL按同/异identity分支复用当前权限。异identity标量0仍表示明确拒绝，只有无记录的NULL才进入原scope；撤权、已知移出/排除及旧空名约束保持。新增另一UUID持有原identity时的真实链回归，见[0024](decisions/0024-current-identity-history-permission.md)。schema7/DTO未变。
- 按用户纠正恢复左导航、右内容，默认首页及首菜单为个人工作台；前3个真实工作区、原资源列表和顶部搜索复用现有状态。工作区、动态、所有文件、收藏和设置独立可达，动态日历保留在其内部。空库突出添加管理位置，不生成演示数据，见[0025](decisions/0025-personal-workbench-sidebar.md)。
- 后续检查发现侧栏截断12项和读取中提前判空。新增4项UI行为回归先实际失败；改为现有滚动侧栏展示全部目标，等待overview后才呈现空范围。home/search各以15个工作区、真实DOM的第13目标和原生事件的第15目标核UUID。首次修后3项失败来自测试固定名称空格及持有已卸载按钮，保留失败日志，修正重新查询及名称前缀后，未降低UUID/数量/拖入/取消断言。
- 最近使用原只按canonical UUID的opened查询，跨identity重指/merge会让B借A的使用记录及时间。3项真实文件回归先失败，修复选择/MAX排序为同一个当前identity来源条件；同identity改名移动保留，web无文件origin仍正常。
- 原生文件选择器返回的整批路径现在先捕获身份，再复用checked导入。确定性在捕获后替换第2项的生产封装回归先失败（误added），修后为added/failed/added，替换对象无资源/授权，其他项及原件字节/mtime保留。丢弃批次也不造授权，选择器取消沿原None提前返回。
- 原生打开从同一Rust快照取得位置/identity/观察名/授权/session/恢复epoch，系统接受后短事务再复验才记opened。跨阶段重指、merge、恢复、撤权、URL变化或路径替换不误记给B；无关revision不拒绝。10项新Core回归实际执行，错误区分系统已接受但活动未记录。见[0026](decisions/0026-current-object-recent-and-open.md)。这不消除系统按路径打开的TOCTOU，不证明宿主完成加载。

失败证据：[最近使用修前](validation/2026-11-20-recent-history-before-026.log)、[选择批次修前](validation/2026-11-20-picked-files-before-026.log)、[侧栏边界修前](validation/2026-11-20-sidebar-boundaries-before-026.log)、[首次修后测试定位失败](validation/2026-11-20-ui-boundaries-after-026.log)。修后定向日志和最终统一检查均保留，不覆盖失败。

### 最终检查与视觉

[最终统一检查](validation/2026-11-20-check-v1-final-026.log)退出0：DTO漂移、TS/Vite、rustfmt、workspace tests及Clippy `-D warnings`通过；**202项Rust、133项UI/API**通过。6项ignored规模测试不计作常规通过。[纽约同组133项](validation/2026-11-20-ui-new-york-final-026.log)本轮实际执行。布局前187/120、后续边界修复前187/129日志分别保留，不当作当前最终检查。

Chrome原生AX访问恢复，正式React+mock IPC夹具完成中文100%目录状态、日历选日/跨月；英文200%长路径/设置；新左右布局中文首页/动态、英文200%首页/设置及250%图标导航/空态/动态。修复侧栏高度不足后设置可达，最后重新载入中文100%有资料首页，范围摘要、三卡及列表正确。[前半阶段视觉记录](validation/2026-11-20-root-location-visual-026.json)与[新布局视觉记录](validation/2026-11-20-sidebar-layout-visual-026.json)分开。浏览器整窗含无关标签，不复制到开源目录；图像在CUA结果中实际观察。15项工作区滚动由DOM/event测试覆盖，未称这组三工作区截图证明15项视觉。

### 正式Core性能，两轮均保留

release磁盘100k混合实体，996搜索页/99,599结果，976时间线页/97,533事件；两轮各**2,240次独立核对、0错误**。均为合成多资源库，不是100k实际文件扫描或原生可见时延。

| 指标 | 第一轮 | 第二轮 |
|---|---:|---:|
| 搜索全链P50/P95/P99 ms | 3.008 / 4.024 / 28.120 | 4.543 / 46.999 / 89.350 |
| 时间线全链P50/P95/P99 ms | 3.278 / 17.764 / 51.839 | 5.091 / 21.593 / 36.665 |
| 首次月历未命中单次API ms | 428.968 | 3407.856 |
| 搜索四深度热P95 ms | 3.199 / 3.311 / 3.249 / 0.962 | 3.531 / 4.526 / 3.562 / 1.915 |
| 时间线四深度热P95 ms | 3.378 / 3.832 / 3.324 / 1.728 | 4.252 / 4.811 / 9.489 / 1.960 |

[第一轮报告](validation/2026-11-20-pagination-modified-first-026.json)/[日志](validation/2026-11-20-pagination-modified-first-026.log)，[第二轮报告](validation/2026-11-20-pagination-modified-final-026.json)/[日志](validation/2026-11-20-pagination-modified-final-026.log)。两轮热搜索P95保持60ms门槛，未放宽；首月3.4秒与尾延迟波动不能隐藏，不能声称稳定首月体验通过。第一轮可能与早期Chrome操作末段重叠，未声称系统无其他负载。第二轮更慢的原因没有确证，不将其确定归因于外部进程。

同SQL分段诊断各16次核对零错：CASE前月历中位583.008ms，CASE后404.641ms，约减少30.6%，见[前](validation/2026-11-20-timeline-profile-before-026.json)/[后](validation/2026-11-20-timeline-profile-after-026.json)。这证明本次诊断的改善，不否定完整API的波动。本轮后续recent及open修复不改search/timeline SQL，未为它们重复上述规模测试；实际10k扫描沿用025，名称/相关性分别保留019/020轮次，不称本轮重测。

### 候选交付及完整V1关口

[026候选ZIP](../deliverables/WorkTwill-v0.1.0-v1-candidate-026-macOS-arm64.zip)已完成[release构建](validation/2026-11-20-build-v1-026.log)，**6,613,848字节**，SHA256 `74a31446435ecc60ec899f2d42f43e647a02d6596dd6d3a32bf8b9a2f6880e43`。CRC、必要bundle条目、执行位、归档binary与当前App一致、正式dist排除review夹具通过。025 ZIP哈希保持。arm64编译器ad-hoc/linker-signed，无Developer ID/TeamIdentifier/公证/密封资源；未安装Applications、无Git提交/push，也未公开发布开源许可。

| PRD交付关口 | 当前状态 | 仍需的实际证据 |
|---|---|---|
| U1–U7原生用户操作 | Core/组件分层回归及浏览器夹具；完整窗口未通过 | WorkTwill真实选择器/IPC/Finder拖入及取消、缩放命中、默认软件、备份恢复、退出重启 |
| U8日常保存与长期取回 | 本机已记录WPS原位保存/另存为+Core重启；不是完整窗口U8 | Word及身份变化安全保存、完整WorkTwill退出/打开、关系和数量稳定且无需人工修复 |
| F14完整可见性能与长期资源 | 正式Core规模与实际扫描证据已有；当前首月波动保留 | 真机输入到结果P95/P99、面板焦点、空闲/洪峰/长期存储与真实查询集 |
| 平台和发行 | macOS arm64本机构建候选 | Windows和所选目录/编辑器矩阵、签名/安装/升级失败回滚、中外环境断网核心流程 |
| 商业/开源退出条件 | 仅已完成调研与候选设计 | 真实目标用户重复任务/再次使用/对照、许可证和贡献授权、正式首发范围与发行决定 |

WorkTwill原生CUA此前明确返回 `Computer Use was not approved to use WorkTwill`，该限制未解除；本轮未重试或借其他工具绕过。Chrome可访问不表示WorkTwill已获准。以上真实关口长期尚缺，不能靠继续累积候选包、单元测试或缩小PRD范围判为交付完成。

## 2026-11-20 迭代027–029：重启后原生诊断与旧数据迁移修复

### 实际原生访问与故障

027将两份系统崩溃报告的binary UUID对应到旧debug程序；用户随后打开明确release路径并确认正常启动。该结论仅解释那两次崩溃，见[027诊断](validation/2026-11-20-native-launch-diagnosis-027.json)。028再次访问仍被拒，记录了用户设置截图所见与实际错误，见[028记录](validation/2026-11-20-computer-use-approval-blocked-028.json)；不是系统权限原因已被证实。

用户回复「都重启了」后，Computer-Use真实读取到了release WorkTwill的`tauri://localhost`首页和截图：显示本地数据操作失败，仍处于读取状态且误呈首次空态。窗口AX偶发cannotComplete；设置点击未形成可确认的原生备份闭环。最终经Computer-Use `super+q` 正常退出，返回App quit。此后工具发现只提供普通node_repl或REA，没有Computer-Use入口；未通过替代GUI桥接继续操作。**当前阻塞是本会话缺少工具入口，不能继续沿用028的应用授权拒绝解释，也不要求用户反复调整Any App。**

只读、WAL-aware结构检查确认实际库`user_version=7`，`activity_root_scopes`缺`retired_location_valid`，目标SQL报no such column。此前同schema7内追加列而遗漏迁移，解释首页读取失败。升级前使用只读源连接执行SQLite Online Backup：746,913,792字节、约4.594秒，schema7/缺列状态已确认；[备份指纹](validation/2026-11-20-pre-migration-backup-029.json)。原始库备份保留在批准临时目录，未放入开源仓库，**不是原生备份UI验收**。

### 修复与回归

- schema8以单个Immediate事务处理已知v7四列/五列格式。四列旧scope缺退休时证据，补列后既有行置0；五列原0/1不变。当前同identity历史按当前授权可见，新capture默认1，retire按实际位置冻结。其他结构损坏仍报错；不改UUID/关系/收藏/观察来源/DTO，见[决定0027](decisions/0027-schema8-history-scope-migration.md)。
- 初始overview未就绪且读取结束时显示短失败文案及「重新加载 / Try again」，结束aria-busy；重试复用原reload，不重新扫描。普通收藏失败仍是操作错误。
- 新增5项真实旧DDL迁移/恢复回归：两种v7格式、当前/退休对象可见性、overview/明细/日历和system筛选、UUID/关系/收藏/原来源保留、新capture/retire、源备份字节不变、触发器失败下ALTER/行/版本一起回滚并可重试。逐表逐列损坏备份拒绝矩阵完整保留，改为明确current schema8。新增3项UI回归覆盖初次失败、重试和普通操作失败。

保留[UI修前红证](validation/2026-11-20-initial-load-error-before-029.log)、[迁移修前5项红证](validation/2026-11-20-v7-migration-before-all-029.log)。首次修后新测试helper误把relinked算入默认文件动态，3项失败，[日志](validation/2026-11-20-check-v1-filter-fixture-failed-029.log)保留；修正helper为默认文件动态与显式system筛选分别核对，未改变产品筛选或删除断言。

[最终统一检查](validation/2026-11-20-check-v1-029.log)退出0：**207 Rust /136 UI/API**通过，DTO漂移、TS/Vite、rustfmt、workspace tests和Clippy `-D warnings`通过。6项ignored不算常规执行。[纽约时区](validation/2026-11-20-ui-new-york-029.log)实际重跑**136/136**通过。本轮未重跑10k/100k规模与Office操作，保持025/026对应轮次和已披露性能波动，不称最新原生性能通过。

### 构建、数据边界与续接

[release构建](validation/2026-11-20-build-native-first-029.log)与[debug构建](validation/2026-11-20-build-debug-029.log)均退出0。旧debug副本先留在独占临时目录，两个本地包均已刷新至schema8代码，避免旧debug再次拒绝新版数据；尚未实际启动新包，不能把构建成功等同原生恢复。

[029候选ZIP](../deliverables/WorkTwill-v0.1.0-v1-candidate-029-macOS-arm64.zip)为**6,614,340字节**，SHA256 `d6f38bd2e69c013afe62573fb632535ffd74cd9b78abb5a4549be1cb9222afbb`。[包检查](validation/2026-11-20-bundle-v1-029.log)通过CRC、必需bundle条目、执行位、归档binary与当前App一致、正式dist排除review夹具；026 ZIP哈希保持。仍为arm64编译器ad-hoc/linker-signed，无Developer ID/TeamIdentifier/公证/密封资源，未安装到Applications或公开发布；无Git提交/push。源码/证据/制品见[029指纹清单](validation/2026-11-20-v1-candidate-029.json)。

6份新合成样本已创建但尚未授权/导入。最后复核size、mtime_ns、inode、SHA256、xattr名称全部与生成基线一致；实际应用库只读复核仍为schema7且缺列，**实际迁移尚未执行**。详见[原生续接记录](validation/2026-11-20-native-handoff-029.json)。下一步必须经Computer-Use打开当前release包，先验证首页恢复和实际schema8，再仅用该合成目录完成授权/取消、中文搜索、内外拖拽、多工作区/收藏、网页标题、文件日历、WPS打开保存/关闭期间保存/重启取回、移除原件不变、排除撤权和原生备份恢复。

完整V1仍未通过：本轮只完成原生故障定位、修复代码与候选构建。Windows、Word及身份变化安全保存、完整原生U1–U8、发行签名/安装/升级回滚、长期资源预算及真实用户重复使用等关口保持，不缩减PRD。

## 2026-11-20 迭代030：实际迁移与部分原生闭环、拖动失败修复

本轮实际使用Computer-Use技能的`node_repl + @oai/sky`，完整release路径避免debug/release同标识歧义。未使用其他GUI桥接。详细结果与合成样本UUID/关系快照见[030原生记录](validation/2026-11-20-native-acceptance-030.json)。

### 实际完成的原生操作

- 启动029新包，先观察加载，再实际显示个人工作台、工作区与资料；旧读取错误消失。只读核查实际库`user_version=8`及缺列已补齐。因此029修复的实际迁移与首页读取在本轮取得证据。
- 创建、改名两个明确标识030的合成工作区，中文经系统粘贴保持；`sky.type_text`初次输入中文只留下030部分，用改名纠正。不把自动化输入结果当作产品输入法缺陷或完整IME通过。
- 打开管理目录原生选择器并取消，原两个目录保持；再次选择时核对GoTo输入和Where都为批准的合成「资料根」，再确认授权。未误选真实工作目录，也未暂停/撤销用户既有两根。
- `⌘K`聚焦搜索框，中文短词「张三」保持；导入前尚未匹配。该步验证快捷键/输入，尚不能称搜索命中矩阵通过。
- 原生选择器实际同时选中合成DOCX/TXT/XLSX，预览显示3 items；提交回执为3项加入、零已存在/未授权/失败，工作区计数3，三个文件可见。保留原始文件UUID用于后续Office取回验证。
- 工作区内只填`https://example.com`，依次看到域名回退、正在获取、Example Domain；修改为中文验收标题后保存，URL规范化为`https://example.com/`、工作区计数4。实际验证可编辑自动标题及原子关联；未称失败网站/关闭获取或默认浏览器矩阵通过。
- 收藏合成合同并勾选，状态显示取消收藏与1项已选择。第二工作区计数仍0。

原生截图在Computer-Use结果中实际查看，包含已有目录或其他资料时未复制整窗截图到开源目录。只在仓库保留合成范围事实，不复制真实业务正文或整库元数据。

### 失败与改动

从选中合同名称拖到第二工作区目标，计数不变、出现跨区域文字选取，**该次内部拖动失败**。`ResourceList -> useResourceDrag -> 原mutation`路径不变；只在资料pointerdown通过guard后阻止默认文字手势，不影响复选框/行操作。新增回归验证单击选择仍正常、拖动松手关联一次且不误触点击，原阈值/取消/失焦回归保留；见[决定0028](decisions/0028-native-resource-pointer-gesture.md)。这些组件测试不能证明WebKit手势修复已成功。

继续访问同一原生窗口时连续3次返回`Sky Computer Use native pipe startup failed`；重置node_repl并按技能重新import后再试一次仍失败。没有改系统设置、结束客户端、强退应用或换工具绕过。**修后原生拖动、Office打开保存/关闭期间保存/重启取回、外部Finder拖入、原生备份恢复未执行**。这与旧轮应用访问拒绝和029工具入口消失分开记录。

### 扫描、检查及制品

合成五文件根最初显示等待，源代码确认扫描轮次串行且dirty合并；后续只读核查已completed、scanned5/failures0、generation2，另有1根仍running。不能把初始等待称永久卡死；本轮未改扫描调度，仍需新目录首批可见延迟的独立测量。6份源样本的size、mtime_ns、inode、SHA256、xattr名称均与029创建基线一致。

[UI定向检查](validation/2026-11-20-ui-drag-030.log)及[最终统一检查](validation/2026-11-20-check-v1-030.log)均退出0：**207 Rust/137 UI/API**通过，DTO、TS/Vite、rustfmt、workspace tests和Clippy告警视为错误。6项ignored不计常规通过；纽约136项属于029，030未重跑。Rust/SQL未变，不重跑已披露026规模性能，也不将组件结果提升为新原生延迟证据。非文件工作区调研沿用已完成的[研究](research/WORKSPACE_CONTENT_RESEARCH.md)，保持file/web范围，简短说明仍为核心验收后的候选，不重复派同题调研。

[release构建](validation/2026-11-20-build-v1-030.log)与[debug构建](validation/2026-11-20-build-debug-030.log)退出0；029 release副本先保留在批准临时目录。[030候选ZIP](../deliverables/WorkTwill-v0.1.0-v1-candidate-030-macOS-arm64.zip)为**6,614,404字节**，SHA256 `98179ff59191f4f2a38eea5e2fe857c48e950cb38b18fa38da06fe620be04996`；[包检查](validation/2026-11-20-bundle-v1-030.log)通过CRC、必要条目、执行位、归档binary与当前App一致、dist排除review夹具，029 ZIP指纹保持。当前构建包含手势修复，不能把此前029窗口结果记为030新进程结果。仍无Developer ID/公证，无Applications安装或Git提交/push。

源码、证据及制品见[030指纹清单](validation/2026-11-20-v1-candidate-030.json)。首要续接是恢复Computer-Use后正常退出旧进程、打开当前新包，验证单项/多项/取消和内外拖放，再继续Office与备份恢复；此前PRD F01–F14/U1–U8及Windows/发行/用户重复使用关口保持，完整V1未通过。

## 2026-11-20 记录031：未保留的月历索引实验与阻塞审计

当前及结束前各重试一次同一WorkTwill完整路径，Computer-Use均返回`Sky Computer Use native pipe startup failed`。这延续前轮无法执行原生验收的实际阻塞；并非新一次应用授权拒绝。未换GUI技术、改系统设置或结束用户客户端。

仍有可独立执行的性能核查，因此没有直接停在状态重述。使用既有正式shared SQL诊断器、release磁盘WAL/FULL、100k混合实体及独立oracle，只在批准临时目录创建夹具：

| 组别 | 月历最小/中位/最大 ms | 查询计划 | 独立核对 |
|---|---|---|---|
| 当前030基线 | 433.174 /539.129 /2944.669 | 来源主键索引，需回表 | 16，零错 |
| 仅新增activity_id/identity覆盖索引 | 403.938 /411.355 /419.026 | 仍使用原主键，新索引未采用 | 16，零错 |
| shared授权查询明确使用覆盖索引 | 440.177 /478.065 /917.832 | 实际COVERING INDEX | 16，零错 |

分别保留[基线日志](validation/2026-11-20-timeline-profile-before-031.log)/[JSON](validation/2026-11-20-timeline-profile-before-031.json)、[未采用索引日志](validation/2026-11-20-timeline-profile-index-031.log)/[JSON](validation/2026-11-20-timeline-profile-index-031.json)、[覆盖索引日志](validation/2026-11-20-timeline-profile-covering-031.log)/[JSON](validation/2026-11-20-timeline-profile-covering-031.json)。16项是四个真实分页位置各4次强制未缓存诊断，不是16次原生首次打开，也不是500k/实际文件扫描。诊断前用于取得真实cursor的分页不计作这16项独立oracle。

首组与次组中位差不能归因于索引，因为次组根本未使用新索引；实际采用覆盖索引后又慢于次组。既有用户目录后台索引负载未隔离，三组波动不能推导稳定改善。**候选索引及INDEXED BY改动均撤回**，未修改权限逻辑或保留额外写入/空间成本。撤回后在文档更新前核验030全部204源码、307原证据及4制品SHA256匹配，无待留运行时代码；既有207/137通过证据仍对应当前运行时代码，不为完整回滚重复全部检查，也不发布031包。详见[结论与回滚证明](validation/2026-11-20-rejected-calendar-index-031.json)。

完整目标仍未满足。优先下一步是取得当前030新进程的实际内/外拖动、默认Office打开/日常及身份变化保存/关闭期间保存/重启取回、日历搜索及原生备份恢复证据，再处理暴露的缺陷。当前Computer-Use连接不可用，其他GUI桥接不能作为替代验收；Windows、签名安装/升级回滚和真实目标用户重复任务也仍缺外部环境或实测条件。阻塞不表示删除这些关口或将候选包改称完整V1。


## 2026-11-20 迭代032：拖动复验、WPS闭环与原生确认归属

本轮通过Computer-Use的`node_repl + @oai/sky`操作当前release；连接恢复，无需用户再次调整授权。详见[032原生证据](validation/2026-11-20-native-acceptance-032.json)。

- WorkTwill原JPEG为2560×1680，消息缩略图约1950×1280。纠正早期未换算坐标的错误后，单合同拖入目标0→1；网页与XLSX两项1→3、回执2成功/0失败；重复拖入无副本；非目标松手不关联，源始终4项。
- 以已保留029包作同条件对照，并只读确认进程实际路径：纪要拖动松手后浮层残留、目标仍3，Esc清理；正常退出并打开030后，同项同坐标目标3→4、浮层清理。支持保留030修复。030原始文字选取证据本身不足，已在[决定0028](decisions/0028-native-resource-pointer-gesture.md)明确更正。原生中途Esc完整矩阵仍待执行。
- 从WorkTwill打开合成DOCX，默认WPS显示原标记；运行中保存增加一标记，正常退出WorkTwill并确认进程0后再保存另一标记；重启WorkTwill并从第二工作区再次打开，WPS画面和DOCX XML均包含三标记。原UUID、inode身份、收藏和双工作区关系保留，其他5份未编辑源保持基线。仅覆盖本机WPS同inode原位保存，不覆盖Word或身份替换。
- 原生⌘K中文快速搜索与完整结果命中合成DOCX/XLSX。工作区动态6条，修改筛选2条、首次发现2条，日历计数相符；选日、跨月空态、回本月保留筛选通过，mtime与观察时间区分显示。
- 通过原生保存面板生成临时目录内`native-backup-032.sqlite3`，822,034,432字节；只读quick_check为ok、schema8、合成收藏及双区关系保留。备份含现有用户元数据，未复制到仓库或开源制品。
- 修前恢复确认未归属WorkTwill窗口，当前AX/截图不可见、busy等待；未选备份或执行覆盖。锁定依赖源码确认无parent走独立系统提醒，parent走窗口sheet。032给恢复、重新指向、合并及拖入授权的四个消息builder设置parent，Tauri注入window，不改DTO/schema或依赖。详见[决定0029](decisions/0029-native-confirmation-window-parent.md)。
- 修后实际启动032，恢复警示sheet可见且AX可读；取消后busy释放。再次确认进入原生备份picker并取消，同样恢复可操作；根授权/暂停/权限版本、合成收藏和关系保持。**未覆盖恢复现有用户库**，其他三种确认仍需原生复验。

运行中WPS保存到file_modified观察约**76.373秒**；关闭期间保存到观察约**554.370秒**（含应用关闭时段），重启后也曾未及时追上。当前notify仅置dirty，2秒检查后排入串行全根扫描，大根会延迟其他已知文件刷新。本轮未改调度，后续须保持授权、身份、恢复epoch与有界资源约束修复，不能把最终一致写成实时通过。

[统一检查](validation/2026-11-20-check-v1-032.log)退出0：**207 Rust /137 UI/API**，DTO、TS/Vite、rustfmt、workspace tests、Clippy通过；纽约136属于029，未重跑。[release](validation/2026-11-20-build-v1-032.log)/[debug](validation/2026-11-20-build-debug-032.log)构建退出0。[候选032 ZIP](../deliverables/WorkTwill-v0.1.0-v1-candidate-032-macOS-arm64.zip) **6,625,940字节**，SHA256 `48fdd661258608a1a489fc3c6116098e46986beed1a2f6339b3a573687c6396a`；[包检查](validation/2026-11-20-bundle-v1-032.log)核对CRC、必要条目、执行位和binary一致、无review夹具。仅ad-hoc/linker签名，无Developer ID、公证、Applications安装或公开发布。

源码/证据/制品见[032指纹清单](validation/2026-11-20-v1-candidate-032.json)。外部Finder拖入、完整恢复、Word/身份变化/目录移动与长期U8、Windows、安装签名/升级回滚及真实用户重复任务仍未通过，完整范围F01–F14/U1–U8保持。

## 2026-11-20 迭代033：已知文件刷新延迟修复与真实保存复验

先冻结032的205项源码、320项证据、4项制品指纹并逐项校验，再进行本轮运行时代码修改。032旧release副本和依赖输入快照保留在批准临时目录的独占子目录，避免将历史验收提升为当前新进程结果。

新增至多256条去重路径通知队列及单个快速刷新任务；Core仅刷新已入库、同身份、当前有权限且未暂停的对象。提交前复核UUID/路径/identity、权限版本和恢复epoch，不创建新授权；删除、移动、替换、未知文件及队列溢出继续由原全量扫描核对。checked导入采用写锁后复读的当前metadata更新投影及活动。见[决定0030](decisions/0030-bounded-known-file-refresh.md)。

[统一检查033](validation/2026-11-20-check-v1-033.log)退出0：**213 Rust/137 UI/API**，含5个Core刷新边界测试和1个有界队列测试；DTO、TS/Vite、rustfmt、workspace tests、Clippy通过。新增测试第一次将数值DTO误当Option而编译失败，修正后通过，保留[首次日志](validation/2026-11-20-known-refresh-first-033.log)与[定向通过日志](validation/2026-11-20-known-refresh-final-033.log)。纽约136及既有规模证据保持原轮次。[release构建](validation/2026-11-20-build-v1-033.log)和[debug构建](validation/2026-11-20-build-debug-033.log)退出0。

正常退出032并只读确认进程0，再经Computer-Use启动033 release（binary SHA256 `d085ddfb291517c5863248adfb697d825f413b47fd270cbfa4b3b35884868b98`）。WPS连续三次实际保存合成合同，文件mtime至file_modified分别**1888.923/1892.975/733.575ms**；三次期间大根仍running。原UUID、同inode身份、收藏和双工作区关系保持，三个新标记及先前标记全部落盘；动态日历无需重新导航自动9→10→11。随后通过WPS关闭已保存合成文档，回到首页。此为三个本机样本，不代表P95、启动补扫、通知洪峰或身份替换安全保存通过。详见[033原生证据](validation/2026-11-20-native-acceptance-033.json)。

032原生网页按钮实际打开Chrome，窗口与地址为Example Domain/example.com。Finder窗口与WorkTwill通过系统窗口菜单移至同屏左右布局后尝试拖入合成独立文件，未取得应用接收证据；不能把不确定的跨窗坐标命中当作产品失败，也不能记为通过。尚未新增独立授权或修改该源文件。

本轮末终端新命令无输出，node_repl纯计算也超时并重置；未确认原因，不归因于WorkTwill、权限或用户环境。随后终端待执行命令恢复返回，未改用户授权或系统设置。033候选ZIP已完成，032指纹只验证032冻结时制品。现有本机033构建及验收证据已保留。完整恢复覆盖、外部拖入、Word/身份变化长期U8、Windows、签名安装/升级回滚及真实用户重复任务仍待完成。

[033候选ZIP](../deliverables/WorkTwill-v0.1.0-v1-candidate-033-macOS-arm64.zip)为**6,633,707字节**，SHA256 `321c8838df4de963a5c254040683e1f105470831451e8e5428ee23c6544f8d7f`；[包检查](validation/2026-11-20-bundle-v1-033.log)通过CRC、执行位及当前release binary一致。当前制品和源码/证据对应[033指纹清单](validation/2026-11-20-v1-candidate-033.json)，无Developer ID/公证或公开发布。


## 2026-11-20 迭代034：新文件及时收录、另存副本与最小原生窗口

033源码208项及4制品在本轮开始时逐项匹配。非文件工作区调研已完成，继续沿用既有研究而不重复派题、不扩展V1为笔记/任务引擎。详细原生结果见[034记录](validation/2026-11-20-native-acceptance-034.json)。

- 从WorkTwill打开原合同，WPS原生面板另存唯一合成副本。原件SHA/大小/mtime/inode与基线一致，副本独立UUID、无收藏及工作区关联。但mtime到首次发现63.695秒，证明033仅刷新已知文件仍不足。
- 034复用有界队列和checked导入，为当前有效、未暂停目录中的新路径收录，提取扫描共用默认跳过规则，保留用户排除及应用数据边界。提交前复核身份、授权、竞争发现及恢复epoch。导入已复验的文件登记当前root generation，避免旧枚举收尾误判missing。没有DTO/schema或依赖变更，见[决定0031](decisions/0031-authorized-notification-discovery.md)。
- 正常退出033并确认进程0，再启动034 release。WPS从已有副本按相同格式另存唯一新文件，mtime到首次发现**2622.434ms**，大根仍running。新路径一条资源、独立UUID、无收藏/工作区继承、原件不变；原生快速搜索命中并高亮新文件。只有一个本机新文件样本，不是P95/洪峰/跨平台SLA。
- 另一次从原合同另存触发WPS格式转换警示，已取消警示和选择器，未执行转换；后续相同格式副本保存成功。Sky曾出现WPS主窗口AX与保存面板截图不一致，按实际应用路径重新绑定后获得正确面板，未将工具上下文差异认定为产品故障。
- 033未变的前端补验：关闭自动标题后手填网址/标题保存；私网标题请求显示拒绝说明后手填保存，两条均关联到合成工作区。未禁用OS网络、未使用流量观察器，不能提升为实际断网或无请求抓包证明。
- 原生窗口经系统菜单及边角拖动缩为**1800×1280物理像素（配置最小900×640逻辑尺寸）**。英文工作区操作和设置入口可达，日历/时间线可读，向上滚动可返回日历。该窗口下合同拖至侧栏目标产生changed0回执；新网页拖入目标4→5、来源仍6。未覆盖OS缩放或完整输入法/无障碍矩阵。
- 034从收藏实际点击原合同打开，WPS窗口标题为原件；合成文档最后关闭至WPS首页，没有处理其他业务文档。

[统一检查034](validation/2026-11-20-check-v1-034.log)退出0：**218 Rust/137 UI/API**，新增5项通知发现/权限/排除/竞争/扫描generation回归；[定向检查](validation/2026-11-20-observed-refresh-034.log)通过。DTO、TS/Vite、rustfmt、workspace tests和Clippy通过；6项ignored不计常规通过。纽约136及既有规模性能保留原轮次。[release](validation/2026-11-20-build-v1-034.log)/[debug](validation/2026-11-20-build-debug-034.log)构建退出0。

[候选034 ZIP](../deliverables/WorkTwill-v0.1.0-v1-candidate-034-macOS-arm64.zip) **6,634,387字节**，SHA256 `43fa3e9ad83c0ba63c593b4ca53588448971873f6e644c6b7eb369b89905f1af`；[包检查](validation/2026-11-20-bundle-v1-034.log)通过CRC、执行位、binary一致及无review夹具。当前源码/证据/制品见[034指纹](validation/2026-11-20-v1-candidate-034.json)，仅ad-hoc/linker签名，无Developer ID/公证、安装或公开发布。

本轮观察同页关联刷新后回到页面顶部；reload清空资料会缩短DOM高度，尚未针对性修复/原生A/B，应保持授权文本失效清除要求继续改善。Finder外部拖入仍未确认接收、完整恢复覆盖未执行；Word/身份变化/跨日/多卷/云目录、Windows、发行安装与升级回滚及真实用户重复任务保持未通过，完整V1尚未完成。


## 2026-11-20 迭代035：清除过期文本与同页滚动位置同时保持

034原生拖入后页面回顶，reload清空资料时DOM高度缩短；既有仅断言未调用scrollTo的测试不能证明实际滚动保持。035记录main几何高度，立即进入loading并清除旧文本；page为空的加载期间临时保留高度，资料到达或读取失败后释放；明确导航同步释放并回顶。没有保留旧业务DOM或另存正式数据。见[决定0032](decisions/0032-reload-layout-height.md)。

新增回归延迟权限变化后的overview，核对旧文本立即从DOM移除、加载占位和高度保持、完成后释放且不调用回顶。[统一检查](validation/2026-11-20-check-v1-035.log)退出0：**218 Rust/138 UI/API**，DTO、TS/Vite、rustfmt、workspace tests和Clippy通过；6项ignored不计常规通过，纽约136及规模证据保留历史轮次。[release](validation/2026-11-20-build-v1-035.log)/[debug](validation/2026-11-20-build-debug-035.log)构建退出0。

正常退出034并确认进程0，再经Computer-Use启动035。英文窗口实际1800×1280物理像素，对应最小900×640逻辑尺寸；滚动至合成合同后重复拖至侧栏归档，取得changed0回执，来源6/目标5保持，合同行仍在约相同可见纵向位置，无页头跳动。明确切换归档工作区后正常回到页头，最后恢复中文个人工作台。详见[035原生记录](validation/2026-11-20-native-acceptance-035.json)。未重跑Office、完整缩放、焦点/无障碍或性能矩阵。

[候选035 ZIP](../deliverables/WorkTwill-v0.1.0-v1-candidate-035-macOS-arm64.zip) **6,634,688字节**，SHA256 `0ab615602ae30046fcbf8a61612623eb19e043f69cbfbd61e7615331a126fccf`；[包检查](validation/2026-11-20-bundle-v1-035.log)通过CRC、执行位、binary一致及无review夹具。当前制品单独对应[035指纹](validation/2026-11-20-v1-candidate-035.json)。仍无Developer ID/公证/安装或公开发布。

Finder外部拖入、完整覆盖恢复、Word/身份变化/跨日及多卷云目录、Windows、签名安装/升级回滚和真实用户重复任务仍未通过。035仅关闭本轮同页滚动问题，不缩减F01–F14/U1–U8，不将完整V1登记为通过。


## 2026-11-20 迭代036：首扫排队与恢复目录后续扫描、原生取回

开始时035全部210源码和4制品哈希匹配。仅在批准临时目录新建独占夹具，两个文件均为合成内容，DOCX复制自既有合成合同，TXT的旧mtime由夹具明确设置。通过原生选择器授权目录A，后续观察仍idle/scanned0且资源0，定位到pick_root只排全量dirty的问题。

036新增最多16个root UUID的FIFO去重队列、固定一个优先任务；新授权、恢复扫描和包含/排除指定目录复用入口，队列满仍由全量核对。首阶段原生全新目录C确认至发现65ms，约0.76秒后界面已完成，另一个大根仍running。中文搜索两份资料、多选原子新建项目/归档工作区，合同收藏，两文件均无独立file grant。

通过终端在夹具文件系统把合同改名并移到子目录，约0.31秒的首次只读观察已更新原UUID/identity，mtime/inode/字节、双区与收藏保持；原生归档界面显示新名称并交给WPS打开。WPS画面出现与落盘XML不同的未保存前缀输入，未保存或丢弃，不能把该画面当作全新磁盘重载证明。

正常退出并确认进程0后，目录A→B。首阶段原生显示匿名占位；只读查明Bookmark已恢复root路径/UUID/identity及别名，但资源路径仍旧A，需要等待全量枚举。该失败保留，没有用重新指向替代自动恢复验收。

后续补修Core位置准备后的通知，未持锁时让Runtime优先处理有效未暂停idle目录，覆盖位置已恢复或中断后的待扫描状态；watcher恢复位置也复用优先队列，暂停目录保持原已知映射核对。仍走scan_root及原授权/身份/代次/epoch校验、批次写锁，不放宽匿名状态规则。详见[决定0033](decisions/0033-bounded-priority-root-scans.md)。

首阶段220 Rust检查及release/debug日志保留为[首次检查](validation/2026-11-20-check-priority-first-036.log)、[首次release](validation/2026-11-20-build-priority-first-036.log)、[首次debug](validation/2026-11-20-build-debug-priority-first-036.log)，旧App在批准临时目录。最终[统一检查](validation/2026-11-20-check-v1-036.log)退出0：**221 Rust/138 UI/API**，含队列容量/退出竞态及Core恢复后可在全量循环前更新同一引用、无持锁死锁的新回归。DTO、TS/Vite、rustfmt、workspace tests、Clippy通过；6项ignored不计常规通过，纽约与规模证据保留原轮次。[最终release](validation/2026-11-20-build-v1-036.log)/[debug](validation/2026-11-20-build-debug-036.log)构建退出0，无DTO/schema/依赖变更。

再次正常退出并确认进程0后，目录B→D，再通过Computer-Use打开最终036。旧工作区中的两文件均更新至D，root与资源UUID/identity、双区与收藏保持，无重复资源、无重新授权/重新指向。通过旧引用打开从未打开的TXT，系统默认文本编辑器AX显示D的真实URL及SYNTHETIC_ONLY_036落盘内容，两文件字节/mtime/inode最终仍与本轮初始基线相同。一次node_repl观察超时后重读同一进程成功；末次ps显示进程启动时间仍为首次启动16:30:42，没有因观察超时重启应用。

最终包另授权全新目录E，确认至发现200ms，约0.86秒后的原生界面显示完成，另一个大根仍running。它与首阶段C是两个小目录样本、不同阶段二进制，不能混记为最终包P95或任意目录SLA。

在归档036的首次发现筛选中，今天11月20日为2条，选择昨日为空，正文保留TXT昨日11月19日10:30修改时间；这是人工旧mtime夹具验证日期语义，不是跨日Office连续性。TXT窗口已关闭，WPS未保存输入仍保留，应用恢复中文个人工作台。完整记录见[036原生证据](validation/2026-11-20-native-acceptance-036.json)。

[036 ZIP](../deliverables/WorkTwill-v0.1.0-v1-candidate-036-macOS-arm64.zip) **6,625,202字节**，SHA256 `e1576fef270f5615a06a56b878a30191b694752597a4968314a229ddbbfc64ef`；[包检查](validation/2026-11-20-bundle-v1-036.log)通过CRC、执行位、binary一致、无review夹具。当前源码/证据/制品对应[036指纹](validation/2026-11-20-v1-candidate-036.json)。无Developer ID/公证/安装或公开发布。

完整Finder外拖、覆盖恢复、暂停/旧路径被替代/多卷云目录的原生矩阵、身份变化/跨日Office、Windows、发行及真实用户重复任务继续未通过。文件系统变化由终端完成，不能替代Finder图形操作。本轮不将有限原生取回提升为完整V1。


## 2026-11-20 迭代037：暂停目录在线维护、权限全链与只读文档标识

开始时036全部212源码/4制品指纹匹配。只在独占临时目录创建两TXT，经原生授权、多选创建工作区、收藏一份，再在设置暂停。终端新增两文件并在线A→B：位置已恢复且仍paused时，三秒后观察已有资源仍旧A，新增资源0、后台大根running；之后真实界面和数据库恢复到B。保留前后观察，不称永久失联。

复用036队列，Core的refresh_root按当前状态选择正式扫描或只按已有UUID分页核对，watcher恢复暂停目录位置也指定维护，不等待全量dirty。另以[首次失败回归](validation/2026-11-20-root-recovery-revision-first-037.log)复现无关网页写入改变revision后，未变的文件/目录恢复被误报permission_changed。文件位置回写现在检查准确目录状态/代次/Bookmark/权限及资源快照，保留实际身份和授权复验；原生目录解析仍检查完整revision。既有撤权/暂停/包含排除/重选/重新指向/恢复负例未删。详见[决定0034](decisions/0034-paused-root-maintenance.md)。

最终[统一检查](validation/2026-11-20-check-v1-037.log)退出0：**224 Rust/138 UI/API**，含新三项回归。DTO、TS/Vite、rustfmt、workspace tests及Clippy通过；6项ignored及纽约/规模历史轮次不重记为本轮。[release](validation/2026-11-20-build-v1-037.log)/[debug](validation/2026-11-20-build-debug-037.log)构建退出0，无DTO/schema/依赖变更。

037真实新进程保持目录暂停，再在线B→C。目录位置确认后约1.66秒的首次完成观察中，两个原UUID已更新C，收藏与工作区保留、没有收录新增文件，后台大根仍running。从原工作区打开TXT，默认文本编辑器AX显示C的真实URL及SYNTHETIC_PAUSED_037_一。暂停时搜新增文件为0；显式继续扫描后才共4文件/新增2，搜索命中。撤销授权后工作区两条匿名引用、全局搜索0；原生选择器重授权同一C后，原root/资源UUID、收藏和工作区保持，已知原件字节/mtime/inode不变、无重复资源。完整[037原生记录](validation/2026-11-20-native-acceptance-037.json)保留各阶段、无独立file grant条件及最后中文工作台状态。文件系统移动由终端完成，不是Finder图形操作验收。

复用已审查探针对033/034实际WPS保存的两个合成DOCX，以两个独立进程共4次读取：Foundation成功返回nil，扩展属性已返回且Document ID=0，指定stat字段和字节哈希不变，无tracking/xattr setter。见[原始结果](validation/2026-11-20-wps-document-id-readonly-037.json)及[研究](research/MACOS_DOCUMENT_ID_READONLY_RESEARCH.md)。这不证明所有卷内部无隐式metadata更新，不把nil视为APFS不支持，也不支持跨inode自动归并。标准/Applications未找到Word，本轮未安装或验收Word。

[037 ZIP](../deliverables/WorkTwill-v0.1.0-v1-candidate-037-macOS-arm64.zip) **6,628,915字节**，SHA256 `5e4a16e0434e9cd5796c9b2a3f47536745e5530384f9bb5e156fcf8815c80664`；[包检查](validation/2026-11-20-bundle-v1-037.log)通过CRC、执行位、binary一致、无review夹具，见[037指纹](validation/2026-11-20-v1-candidate-037.json)。仍为ad-hoc/linker签名，未签名发行、安装或公开发布。

以上为小目录有限场景，不代表大量暂停映射或全部目录类型SLA。完整Finder外拖、覆盖恢复、Word/身份变化/跨日Office、替代旧路径/多卷云目录、Windows、发行和真实用户重复任务仍未通过；WPS既有合成文档未保存输入继续保留。


## 2026-11-20 迭代038：原生搜索补验、扫描版本与分页稳定

原生搜索补验仅使用已批准临时根下的合成资料。范围从全局2文件切到目标工作区1文件；中文路径前缀、文件类型与范围组合正确；明日开始日期排除样本、同日区间包含晚间mtime，倒置日期明确报错；英文空态、关键词替换、Esc关闭及名称/最近修改控件取得实际证据。两文件mtime相同，不据此证明不同时间排序；Esc可见结果也不提升为捕获某个SQLite中断。见[搜索状态](validation/2026-11-20-native-search-038.json)。

对037合成目录撤权后，收藏不再显示原名，工作区保留两条匿名引用，工作区动态/首次发现日历为0；原生选择器重授权后，原名、收藏及两条首次发现恢复。只读核对原root/资源UUID、收藏和关系一致，四份原件字节/inode/mtime不变，见[权限记录](validation/2026-11-20-native-permission-038.json)。038外拖记录曾被JS JSON往返舍入纳秒整数，已按最初Python捕获值恢复并改为十进制字符串，原件实际时间未变；不修改原件来通过检查。

103项分页先在037两次stale_cursor回首页。扫描批次无内容变化仍递增revision，同根硬链接还轮换名称/位置；独立回归先失败。038仅在查询事实变化时递增版本，保留仍在当前根且未排除的同物理身份有效路径；真实修改/失联/范围与权限仍使旧游标失效。无变化完成通知分为scan-progress，保留搜索后页；真实变更仍同步清旧授权文本，迟到进度受读取代次约束。100项完整首页即进入同一虚拟滚动区，避免续页才切换布局。决定与代价见[0035](decisions/0035-catalog-query-stability.md)。

最终038新进程连续遍历000–102的103个不同文件名，末页无加载更多，逐段没有重复行。原图2560×1680坐标点击续页后094–099位置保持，100–102追加；此前AX按钮调用显示000的观察单列，不据此继续判产品回顶。长间隔后其他真实资料写入仍可能清页；未记录那些间隔的前后版本，不杜撰无变化或并发快照保证。见[连续遍历](validation/2026-11-20-native-pagination-038-continuous.json)、[鼠标坐标复验](validation/2026-11-20-native-pagination-038-coordinate.json)及相邻截图。分页夹具已撤权，103份文件保持基线，外拖样本未改变。

统一检查退出0：227 Rust、140 UI/API及DTO/TS/Vite/rustfmt/Clippy；最后只改100项虚拟边界后最终UI141项和release的TS/Vite再次通过，未无故重跑Rust或debug包。6项ignored不计常规通过；另明确执行10k真实合成文件扫描验收1项，三个阶段逐项metadata/关系/事件核对通过。初扫4.705秒，无变化/单修改重扫0.692/0.684秒；并发搜索P95为4.131/2.928/2.047ms，首结果约30.3ms，仅Core实际合成文件测量。见[统一检查](validation/2026-11-20-check-v1-038.log)、[最终UI](validation/2026-11-20-ui-final-038.log)、[10k报告](validation/2026-11-20-scan10k-038.json)。首轮测试请求包装编译错误、两项修前回归及虚拟边界失败、中间构建和未完成遍历均保留，不删除失败检查。

[038本地候选ZIP](../deliverables/WorkTwill-v0.1.0-v1-candidate-038-macOS-arm64.zip)为6,631,094字节，SHA256 `6e655f177125edabb6523aef7be457beb038262cbcd895b61630eb82a10331e8`；CRC、执行位、binary与当前bundle一致、无review夹具通过，037旧包SHA保持。仅ad-hoc/linker签名，无Developer ID/公证或安装发布；未执行Git。见[包检查](validation/2026-11-20-bundle-v1-038.log)、[038指纹](validation/2026-11-20-v1-candidate-038.json)及[原生总结](validation/2026-11-20-native-acceptance-038.json)。Finder外拖仍未通过，已恢复同屏左右目标1/来源2的交接状态；Mac核心剩余按有限清单继续，Windows后续真机独立验证，整个V1不登记完成。


038签名检查边界：本机原生应用已实际运行；`codesign --verify --deep --strict` 对当前bundle退出1，报告 `code has no resources but signature indicates they must be present`。此项未通过，不将ZIP结构/运行检查当作签名、安装或公开发行通过。按PRD §9.0，签名及相应公开发行准备后置；见[实际检查记录](validation/2026-11-20-codesign-038.json)。
