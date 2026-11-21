# Windows V1 工程候选交付

2026-11-21。按用户最新要求优先验证日常使用，不继续扩大极端场景。当前可试用工程候选；真实跨日等未执行项不登记通过，完整V1目标未标完成。

## 使用与制品

- EXE：`deliverables/WorkTwill-v0.1.0-windows-x64-candidate-narrow-search.exe`
- ZIP：`deliverables/WorkTwill-v0.1.0-windows-x64-candidate-narrow-search.zip`
- EXE SHA256：`96D39A256F3F1EBA09870A266DECAAF672750DC5613042FD949778A5FC6B95E1`
- ZIP SHA256：`14ACBF96A90EC610E21FCDCA8CB5ECB884ED1FAD0C1373BD4870EA76974CB739`
- ZIP内EXE摘要一致，见validation/2026-11-21-windows-narrow-search-zip-check.json。
- 未签名便携程序，依赖WebView2。正常启动使用系统应用数据目录；验收隔离环境变量仅供独立夹具使用。
- 基线9124cc6，交付包含工作区未提交修改；没有提交或推送。

## 实测结果

环境为Windows 10 LTSC 21H2 build19044 x64、C盘NTFS普通本地目录、WPS12.1.0.29690、125%DPI。

最新96构建已执行真实WPS GUI运行中保存、关闭WorkTwill期间保存、同EXE重启，分别关闭文档后从两个工作区与收藏重新打开正文。新标记落盘并在窗口可见，原UUID、收藏、双区关系保持，同名资源1。证据96-running/offline-body、96-assertions、96-A/B/favorites-reopened。

最新构建修复窄窗搜索面板左缘裁切，900×640物理窗口英文长文件名搜索截图通过；150字符文件名、273字符路径可搜索并发起系统打开。记事本正文UIA抓取失败，不称该项正文读回通过。

相邻B4构建完整检查146 UI/API、201 Rust通过，DTO、fmt、严格clippy及build通过；96仅变更CSS并完成release和对应原生复验，未重复全量。B4的100k合成元数据正式原生UIA测量中文P95 149.07ms、英文空闲P95 134.92ms；英文并行完整检查轮179.80ms超预算保留。观测点是UIA非屏外结果出现，不是像素呈现。原生SQLite执行中断24次且后续查询正常；固定前台聚焦30次P95 15.96ms。

此前58构建实际GUI另存为独立和撤权各入口隐藏通过；316原生离线关闭追踪及同名重建保守拒绝通过。完整证据分别归原构建，不能冒称96全部重跑。

## 未验证与后续

- 真实跨日保存取回、Microsoft Word未验证；WPS COM不计Word。
- 临时替换源已登记UUID时保持保守拒绝，可能需要用户确认。按用户要求不继续追逐低概率时序；不自动合并或退休记录。
- 扩展DPI/极端压力组合、像素呈现时间不继续扩大；并行负载超预算结果保留。
- 签名、安装升级、公开发行准备和完整备份恢复矩阵按原批准决定后置。

详细记录见WINDOWS_V1_ACCEPTANCE.md末尾、WINDOWS_V1_SUPPORT_MATRIX.md及validation/2026-11-21-windows-narrow-search-candidate.json。上述后续项不影响本次工程候选交付，未验证事实保持可见。
