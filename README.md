# WorkTwill

本地个人工作台：快速找到文件和网页，围绕工作事项组织引用，保持原文件原地不动。项目已确定完整开源，并探索官方发行与支持服务等商业方向，已进入原生产品开发，当前为 V1 验收候选，尚未完成全部原生与发布验收。

**当前文件夹就是代码目录。** 文档、交互原型、实验工具和可复查结果保存在这里；会被改名、替换或删除的测试文件放在单独确认的临时样本目录。

| 路径 | 当前用途 |
|---|---|
| [PRD.md](PRD.md) | 产品范围、用户场景和验收 |
| [ARCHITECTURE.md](ARCHITECTURE.md) | 服务职责、数据模型、接口、原生验证与 AI 开发维护契约 |
| [开发状态](docs/DEVELOPMENT_STATUS.md) | 当前实现、检查证据、缺口与下一步验收 |
| [验收与迭代记录](docs/ACCEPTANCE.md) | 必需验收关口、真实 WPS 保存与 Rust 规模测试结果 |
| [desktop](desktop/) | 正式 React/TypeScript 桌面界面 |
| [website](website/README.md) | 独立中文官网、使用说明、直接下载与 Cloudflare 发布配置 |
| [src-tauri](src-tauri/) | Tauri 原生入口、命令权限、选择器与系统打开 |
| [worktwill-core](crates/worktwill-core/) | Rust 领域逻辑、SQLite、只读扫描、授权和搜索 |
| [index.html](index.html)、[app.js](app.js)、[styles.css](styles.css) | 浏览器交互原型，使用本地演示状态；不是正式桌面后端 |
| [prototype-b](prototype-b/index.html) | 另一版交互原型 |
| [spikes](spikes/README.md) | 独立 Python/Swift 技术实验，不作为正式产品 API |
| [实验结果](spikes/RESULTS.md) | 已执行、未完成及失败边界与原始证据 |
| [文件跟踪调研](FILE_TRACKING_RESEARCH.md) | 竞品与原生 API 依据，保留调研时版本说明 |

## 直接使用与开发入口

**2026-11-21 当前安装更新候选：** [Mac Apple 芯片 DMG](deliverables/WorkTwill-1.3.1-darwin-aarch64-944289286057-WorkTwill_1.3.1_aarch64.dmg)。应用设置已补安装更新帮助；[双语离线安装说明](docs/INSTALLATION.txt)说明首次系统确认和后续更新。按用户决定暂缓付费系统签名，保留更新包验签。官网源码已更新，本轮未部署；下面旧候选与验收描述保留为历史。构建与实际验收边界见[安装更新](docs/INSTALL_AND_UPDATES.md)。

macOS arm64 应用位于 `target/release/bundle/macos/WorkTwill.app`，可通过 Finder 打开。应用本身不需要安装 Node.js、pnpm 或 Rust。当前包没有 Developer ID 签名和公证。最新压缩包为 [迭代038候选](deliverables/WorkTwill-v0.1.0-v1-candidate-038-macOS-arm64.zip)。实际原生已验证首页/数据迁移、内部单项与多项拖动、WPS原位保存及关闭应用后保存再取回、中文搜索与部分日历流程；032补修确认框的窗口归属。033已补已知文件快速刷新并取得三次WPS保存证据；完整原生矩阵、启动补扫/更多刷新负载、Windows与发行仍未通过，见[验收记录](docs/ACCEPTANCE.md)和[使用说明](docs/USER_GUIDE.md)。

当前源码采用左侧导航、右侧内容结构：默认进入左侧首项「个人工作台」，其下是工作区、动态和收藏；全局搜索位于右侧内容顶部。首页复用资料列表并展示前3个真实工作区，日历时间线只在动态内部。窄窗口保留主导航图标栏，侧栏高度不足时可纵向滚动，空库主要入口为明确授权管理位置。2026-11-21按用户决定删除「所有文件」独立入口与页面，收藏固定展示收藏资料；首页资料和完整搜索结果保留，见[决定0037](docs/decisions/0037-remove-all-files-page.md)。旧候选包及[布局决定0025](docs/decisions/0025-personal-workbench-sidebar.md)保留历史版本，不代表本次源码变更已经打包或原生验收。

已实现原生/应用内拖入、网址自动标题（可关闭/编辑）、文件日历时间线、完整搜索筛选、工作区排序及批量逐项反馈；024接入macOS新授权目录的同物理身份改名/同卷移动定位和暂停已有引用恢复。旧管理目录需明确重选后建立身份锚点，物理替换不猜测继承。授权与原件安全边界见 [验收记录](docs/ACCEPTANCE.md)。

025修复历史名称和同路径身份状态，增加历史来源授权、合并审计及重新指向影响确认；最终186项Rust、120项UI/API及纽约120项通过。10万混合实体分页2,240核对零错；首次月历约624ms，1万真实合成文件初扫两次为4.953/11.374秒，性能差距保留原始记录，不作为稳定原生体验保证。

026完成上述左右布局与工作台默认首页；补齐全量侧栏拖放目标及授权读取状态，并修复当前对象的最近使用、选择文件批次身份和打开活动快照复验。当轮202项Rust、133项UI/API及纽约同组133项通过。10万混合实体两轮完整分页各2,240核对零错；首次月历约429/3408ms，波动如实保留。旧026 ZIP及[指纹](docs/validation/2026-11-20-v1-candidate-026.json)保留为历史，不能验证更新后的App。

029在重启后的真实原生首页发现读取失败，确认旧schema7缺列；升级为schema8，以事务补齐迁移并保留关系与来源，同时补上首次读取失败的重试状态。统一检查207 Rust/136 UI、纽约136项通过；release和debug均已重新构建。已通过只读源连接保存升级前备份，尚未重新打开新包执行实际库迁移。原窗口曾成功读取并正常退出，之后当前会话缺少Computer-Use入口，完整原生验收仍未完成。详见[决定0027](docs/decisions/0027-schema8-history-scope-migration.md)、[开发状态](docs/DEVELOPMENT_STATUS.md)及[验收记录](docs/ACCEPTANCE.md)。

030恢复Computer-Use后已实际打开029新包，首页正常、实际库升级schema8；合成目录授权/取消、原生三文件多选导入、网址自动标题并修改保存、收藏与工作区改名完成。内部拖动实际失败，已补默认手势抑制，统一检查207 Rust/137 UI通过；release/debug已更新。随后原生连接连续报native pipe启动失败，修后拖动及Office/外部拖入/备份恢复仍未验收，不将部分通过称为完整V1。见[030实际记录](docs/validation/2026-11-20-native-acceptance-030.json)与[决定0028](docs/decisions/0028-native-resource-pointer-gesture.md)。

032纠正此前缩略截图坐标误用，重新做原029/当前030同文件同坐标对照：原实现松手后残留浮层、未关联，030成功。单项、多项、重复及非目标松手均实测；合成合同经WPS两次原位保存（一次WorkTwill进程为0），关闭文档/重启应用后从第二工作区重新加载，内容、UUID、双工作区和收藏保留。原生备份保存及完整性检查通过；恢复确认未绑定窗口的问题已修复，实际窗口sheet和两级取消通过，未覆盖恢复现有用户库。207 Rust/137 UI通过；日志、受限范围与扫描延迟见[032实际记录](docs/validation/2026-11-20-native-acceptance-032.json)。

以下用于源码开发：

需要 Node.js 22 LTS 或兼容的较新版本、pnpm 10、Rust（版本由 `rust-toolchain.toml` 固定）及对应平台的 Tauri 构建环境。当前仅在 macOS arm64 编译验证。

```sh
pnpm install --frozen-lockfile
pnpm desktop
```

首次启动进入「个人工作台」，没有演示数据，也不自动扫描用户目录。桌面、下载、文档建议只设置系统选择器起点，确认所选目录后才授权扫描。在个人工作台空态点击「添加管理位置」，或通过「管理位置与设置」明确选择目录后才建立索引；也可只授权选中的文件。当前保存追踪仍为保守开发策略，见开发状态；日常文件测试应先使用副本。

```sh
# 仅查看界面，无原生文件访问能力
pnpm dev

# 检查：仅传入已确认允许创建和删除测试样本的临时目录
pnpm check --fixtures /private/tmp/worktwill-spikes-20261119

# 实际合成文件扫描基准，只在已确认的临时目录内新建独立样本
WORKTWILL_TEST_ROOT=/private/tmp/worktwill-spikes-20261119 pnpm bench:scan

# 显式扩大到10万个合成文件/1000子目录（样本生成不计入扫描耗时）
WORKTWILL_TEST_ROOT=/private/tmp/worktwill-spikes-20261119 WORKTWILL_SCAN_FILES=100000 pnpm bench:scan

# 合成多资源磁盘库的时间线与完整搜索分页（不创建真实办公文件）
WORKTWILL_TEST_ROOT=/private/tmp/worktwill-spikes-20261119 pnpm bench:pagination

# macOS 本地 release 包（不含 Developer ID 签名/公证）
pnpm tauri build --bundles app

# 调试构建
pnpm tauri build --debug --bundles app
```

macOS release 产物为 `target/release/bundle/macos/WorkTwill.app`，调试包位于 `target/debug/bundle/macos/WorkTwill.app`，不会自动安装到系统应用目录。正式业务数据由 Rust 写入系统应用数据目录；macOS 为 `~/Library/Application Support/app.worktwill.desktop/`。备份不包含源文件，恢复后需要重新授权管理位置。

保留原型仍可独立查看：

```sh
python3 -m http.server 8000 --bind 127.0.0.1
```

浏览器访问 `http://127.0.0.1:8000`。原型不证明原生文件打开、长期身份跟踪或搜索性能。实验运行方式见 [spikes/README.md](spikes/README.md)。

## 开发与验证边界

正式运行时使用 Tauri 2、React/TypeScript、Rust 和 bundled SQLite，DTO 由 Rust 生成，业务数据不存入浏览器 localStorage。工程约定见 [AGENTS.md](AGENTS.md)，已实施决定见 [原生开发基线](docs/decisions/0001-native-development-baseline.md) 、[拖入、网页与时间线](docs/decisions/0002-v1-drag-web-timeline.md) 、[查询取消与混合分类](docs/decisions/0003-search-cancellation-and-groups.md) 、[本地日历汇总](docs/decisions/0004-bounded-local-calendar.md) 、[版本化搜索分页](docs/decisions/0005-versioned-search-pagination.md) 、[扫描投影与提交复验](docs/decisions/0006-scan-projection-and-current-metadata.md) 、[界面语言与输入状态](docs/decisions/0007-ui-language-and-input-states.md) 、[扫描读取并发与恢复代次](docs/decisions/0008-scan-read-concurrency-and-restore-epoch.md) 、[首次常用位置](docs/decisions/0009-suggested-managed-locations.md) 、[双语失败反馈](docs/decisions/0010-bilingual-error-feedback.md) 、[所选资料创建与搜索上下文](docs/decisions/0011-selected-workspace-and-search-context.md)、[多资源分页验收](docs/decisions/0012-multi-resource-pagination-acceptance.md) 和 [备份快照与忙锁](docs/decisions/0013-backup-snapshots-and-busy-errors.md)。

本地文件/关系/搜索无需联网；仅主动开启的网页标题获取访问目标网站与固定 Cloudflare DoH。可以关闭自动标题，失败也可手填。

核心测试通过不代表原生界面、Word/WPS 保存、Windows、签名发行和商业价值已验收。开发状态保留 F01–F14 缺口，后续必须完成“授权→扫描→搜索→加入工作区→打开→保存→重启取回”的目标软件实测。许可证、价格和首发平台尚未决定；仓库目前没有授予外部使用的开源许可证。

033修复大目录扫描期间已知文件刷新排队：有界去重通知队列、单个快速任务，提交前复核身份、权限、暂停和恢复代次，未知/移动/替换保持全量扫描核对。213 Rust/137 UI通过，release/debug已构建。新release中三次实际WPS保存到修改事件约1.889/1.893/0.734秒，大根仍在扫描，收藏及双区关系保持，日历自动更新；只有三个本机样本，不代表P95或完整V1通过。网页按钮实际打开默认Chrome；外部Finder拖入仍未确认接收。见[033原生记录](docs/validation/2026-11-20-native-acceptance-033.json)与[决定0030](docs/decisions/0030-bounded-known-file-refresh.md)。

034将有界通知通道扩展到已授权、未暂停目录中的新路径，共用自动扫描的跳过/排除规则，checked导入复核权限和恢复代次，并登记当前扫描generation以免旧收尾误报失联。218 Rust/137 UI、release/debug通过；WPS同格式另存新副本约2.622秒自动收录，修前同机样本约63.695秒；副本新UUID、无收藏/工作区继承，原件不变。补验了关闭自动标题及标题失败手填保存、英文900×640原生布局、最小窗口侧栏拖动与收藏打开原件。一个新文件样本不能代表P95；Finder外拖、完整恢复和发行等仍待完成，同页刷新滚动位置也仍需优化。见[034记录](docs/validation/2026-11-20-native-acceptance-034.json)和[决定0031](docs/decisions/0031-authorized-notification-discovery.md)。

035修复同页安全重读引发的滚动跳动：立即移除旧资料文本，加载期间只保留main几何高度，资料到达后释放，明确导航仍回顶。218 Rust/138 UI/API及双构建通过；真实英文900×640窗口中重复拖入后列表位置保持、切换工作区正常回顶。见[035原生记录](docs/validation/2026-11-20-native-acceptance-035.json)和[决定0032](docs/decisions/0032-reload-layout-height.md)。完整V1关口继续保留。

036补修新授权及已恢复位置目录等待其他大根扫描的问题：指定目录FIFO队列最多16项、固定一个工作任务与原全量任务并行，Core在位置准备完成后通知原生层优先处理idle目录，复用既有权限/身份/代次/恢复屏障和批次写锁。最终221 Rust/138 UI及双构建通过。最终包全新小目录确认后200ms产生发现记录；合成目录在应用退出期间移位，重启后保留root/资源UUID、双区及收藏，通过旧引用在默认文本编辑器读到新目录D的实际URL和落盘标记。文件系统变更由终端完成，不能替代Finder拖入验收；WPS未保存输入保持未处理，完整Office/Windows/发行关口仍保留。见[036原生记录](docs/validation/2026-11-20-native-acceptance-036.json)和[决定0033](docs/decisions/0033-bounded-priority-root-scans.md)。

037将暂停目录的已有引用核对接入同一优先维护队列，继续不枚举新增文件；修复无关业务revision阻止文件位置回写的问题，保留目录/权限/资源/物理身份及目录解析CAS检查。224 Rust/138 UI及双构建通过。实际暂停在线移位后旧引用更新到C、未收录新增文件；明确恢复扫描后才收录，撤权匿名隐藏，重新授权恢复同UUID/收藏/工作区，源文件不变。另补实际WPS保存合成文件的只读Document ID探针，仍为nil/0，不据此支持跨物理替换。见[037记录](docs/validation/2026-11-20-native-acceptance-037.json)和[决定0034](docs/decisions/0034-paused-root-maintenance.md)。


038修复实际原生分页暴露的扫描无变化版本递增和同根路径轮换，并区分扫描进度通知，完整首页100项即使用同一滚动区。227 Rust/最终141 UI、release及10k真实文件扫描检查通过；原生连续遍历103个不同文件名，坐标点击续页保留末尾位置。另补搜索组合筛选、中英文反馈及撤权后收藏/日历隐藏。Finder外部拖入仍待手动手势确认，当前不是Mac核心验收完成或双平台首发通过。见[038结果](docs/validation/2026-11-20-native-acceptance-038.json)及[有限工程清单](docs/CURRENT_ENGINEERING_CHECKLIST.md)。


### V1.3.1 本地 Mac 更新

正式运行时为 desktop/src-tauri/worktwill-core；本轮删除“所有文件”，首页最近修改与真实最近打开分离，新增大型快捷搜索、完整搜索筛选页、静态伙伴和SQLite纯文本便签。名称单击打开、复选框选择。使用、制品路径和本轮实际结果见 [V1.3.1 Mac 交付记录](docs/V1_3_1_MACOS.md)。不把旧包或浏览器原型视为新版，也不将Mac结果记为Windows通过。
