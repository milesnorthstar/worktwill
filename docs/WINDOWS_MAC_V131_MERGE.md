# Mac V1.3.1 与 Windows 连续性合并

2026-11-21，按用户要求拉取并解决冲突。main已快进至origin/main f0d9f0aa1f0775ad991574ac151a5c41be02530b（含9080ec5便签/工作台/搜索优化）。未提交的Windows工作在更新前以stash完整保存：08be0ead26fc7696e857175b13c81f09ff419a59，恢复后仍保留备份，没有drop。

已解决9个文件冲突，保留Mac便签、退出保存、搜索/首页和Windows追踪同意、连续性、权限/历史判断。DTO由pnpm types生成。两端schema9含义不同，统一到schema11，支持Mac9和Windows9/10，事务内补齐校验且保留原数据；沿用升级前在线快照。新增三种旧布局的数据保留回归。最近打开时间及推荐豁免统一使用连续性授权链，避免自动合并后遗漏。

检查：154 UI/API通过；Rust全套208通过，之后新增迁移回归1项通过；严格clippy、fmt、DTO检查、前端build和Windows release构建通过。首次编译遗漏旧常量引用失败已修复，原日志保留。无冲突标记与未解决索引项。未新建提交，未推送；修改留工作区，冲突文件已标记解决。

Windows原生合并版在独立临时库v131-merge-native下启动PID48388，UIA确认新版首页/搜索导航；未把启动冒充完整V1.3.1原生验收，未改旧验收数据库。Mac原生构建/验收仍需Mac本机执行。

制品：deliverables/WorkTwill-v1.3.1-windows-x64-merged.exe 与同名ZIP。
EXE SHA256：935608233A9364E5EEF0405B33E3370EAD3B422B3C472E83A3564C87E5C754AB
ZIP SHA256：F2395FCDE892773CB70E76248A3A861AE0A7E0FDB704BE4FB45F8161CE98FF50

2026-11-21提交补充：用户已明确要求提交所有必要修改。本次将合并源码、schema11迁移/回归、DTO、权限、文档及可追溯验收证据一并提交到本地Git；上文“未新建提交”描述为此前合并阶段状态。deliverables与.tools按既有gitignore保留本机，不强制加入仓库；不推送，stash安全备份继续保留。
