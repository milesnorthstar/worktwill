# Windows V1 工程验收 · 2026-11-21

结论：**工程检查、Windows x64 release 构建及部分原生流程通过；完整 Windows V1 未通过。** WPS 安全保存改变物理身份后，原引用需人工确认，U8明确失败。没有放宽原件只读、身份或权限要求。

## 环境与制品

- 最新代码已从 `81fec63` 快进到 `e055f72`，本轮修复尚未提交；没有 push。
- Windows 10 Enterprise LTSC 2021，21H2 / 19044，Intel i5-1155G7，x64；仅本机内置磁盘普通目录。
- Node 22.23.4、pnpm 10.33.2、Rust 1.96.0 MSVC、MSVC 14.44.35207、WebView2 157.0.4341.60。安装微软 C++ 工具链；项目依赖按锁文件安装，未升级锁文件。
- 当前候选：[日期修复版EXE](../deliverables/WorkTwill-v0.1.0-windows-x64-candidate-date-fix.exe) / [ZIP](../deliverables/WorkTwill-v0.1.0-windows-x64-candidate-date-fix.zip)。无安装器/签名，需 WebView2；不是正式发行包。旧候选保留历史制品。
- SHA256：`1B8BB529835767D4EF33828F2EC3A51CE8919C6D98255F895E023884353BCF33`，见[制品清单](validation/2026-11-21-windows-candidate.json)。
- 专用临时根：`C:\Users\Administrator\AppData\Local\Temp\worktwill-windows-v1-20261121`；每组夹具独立创建，不清空已有目录，不使用用户业务文件。

## 修复与检查

1. `windows-sys` 补 `Win32_Security` 特性，解决 `CreateFileW` 在 Windows 无法编译。
2. 检查脚本兼容 `pnpm.cmd`，DTO 漂移检查仅统一 CRLF/LF，仍逐字比较声明。
3. Bookmark 私有实现按 macOS 编译，Windows clippy 无死代码告警。
4. Windows 盘符绝对路径前缀做纯词法转换以匹配入库规范路径，不访问或授权输入路径。重新指向、改名和规模夹具按正式规范路径核对；不删除失败断言。
5. 打开复验、历史权限、审阅/取消/过时合并及替换恢复共30项测试新增 Windows 执行；整目录自动定位依赖 Bookmark 的单项保持 macOS 限定，Windows该能力未支持。

最终 `pnpm check --fixtures <上述临时根>`：**144 UI/API、183 Rust通过，5个显式规模测试默认忽略**；含DTO、生产前端构建、fmt、workspace test、严格clippy。见[最终日志](validation/2026-11-21-windows-check-date-fix.log)。忽略项中另显式执行扫描及完整分页，结果见下；其他未运行不登记通过。原始编译、换行、路径及夹具失败日志保留。

## 实际原生结果

通过 Windows UI Automation、WebView2 可访问性和真实鼠标/键盘操作，未注入模拟IPC。启动时为自动化设置 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--force-renderer-accessibility`，产品不依赖该参数。

| 项目 | 实际结果 / 边界 |
|---|---|
| release 启动、正常关闭再启动 | 新PID 28660，原生窗口响应，已管理目录、原工作区和收藏重新可见 |
| 目录授权与初扫 | 原生选择器选择专用普通目录，5个中文TXT可见；无默认全盘扫描 |
| 组织与转移 | 两TXT加入A；第1份同时关联A/B并收藏，第2份转移到B；A/B计数及文件显示符合提交结果 |
| 顶部搜索 | 原生输入“合同”返回5项；Esc关闭；未捕获原生SQLite中断，不计中断端到端通过 |
| 网页 | 英文界面输入URL，自动取得Example Domain，改标题保存到A |
| 中英文 | 中英界面实际切换及可访问性树，中文搜索截图；未完成最小窗口/各DPI完整矩阵 |
| 动态 | 实际月历显示2026年11月21日7条记录，文件修改/发现/打开记录；跨月/全筛选仍未完成 |
| 撤权 | 撤销目录后收藏空、搜索“合同”无匹配，工作区保留匿名占位；重授权恢复文件和原工作区/收藏 |
| Explorer单项 | 真实跨窗拖第3份TXT至A，A由2变3，导入回执出现 |
| Explorer多项/重复 | 第3、第4份TXT一起拖入：1加入、1已存在，A变4 |
| 未授权单文件 | 第6份无授权时回执1未授权；原生确认取消未增加计数；重试确认授权后A变5、仅独立文件授权 |
| 混合授权拖入 | 校准后同批得到1已存在/1未授权/0失败；取消后A保持6项，重试授权后A变7项，仅增加第8文件独立授权，目录保持撤销。此前误中导航的失败保留；三类同批中包含新加入已授权项的分支仍未覆盖 |
| 原件安全 | 早期5TXT核对不变；最终第1份被验收Notepad快捷键误编辑，第2–5份仍不变。见末尾纠正与最新[核验](validation/2026-11-21-windows-source-integrity.json)，不能宣称全程5份不变 |
| 默认DOCX打开 | WorkTwill默认打开真实WPS窗口 `Office验收.docx - WPS Office`，显示合成原始标记 |
| WPS保存 | **失败**，见下，不用正常启动/打开代替保存连续性 |

[中文窗口](validation/2026-11-21-windows-native.png)、[授权确认](validation/2026-11-21-windows-native-authorization.png)及同日期原生UI日志保留实际观察。

## WPS U8失败复现

1. 正式release内合成DOCX原UUID `883786d7-f5a0-474d-9a75-03dc7f188ba6`，收藏且关联A/B。
2. 从WorkTwill打开，实际WPS `12.1.0.29690`，在文档正文输入 `WINDOWS_NATIVE_WPS_SAVE_1`，Ctrl+S。
3. 只读打开ZIP XML确认标记已落盘。原UUID变`identity_unconfirmed`；另一UUID `89bc0db4-8251-4acd-9455-0eedaf674bd0`代表新物理对象，无收藏或原工作区关系。创建时间相同而File ID不同。
4. 只读 `fsutil objectid query` 返回没有Object ID。没有调用创建Object ID、写ADS/xattr、正文哈希归并或同路径自动换绑。
5. 另一个独立夹具通过 `Word.Application` COM也复现；查注册表确认此COM实际指向WPS，不能登记为Microsoft Word测试。COM退出有RPC错误，单列；正式界面Ctrl+S结果为独立复现。

见[正式库快照](validation/2026-11-21-windows-wps-native-save-failed.json)、[Object ID查询](validation/2026-11-21-windows-wps-objectid.log)、[独立夹具](validation/2026-11-21-windows-office-after-save.log)。此结果证明现有Windows File ID方案不满足该WPS版本日常安全保存连续性；人工重新指向不算U8成功，不能宣布已支持。Microsoft Word、跨日、另存为副本及退出期间保存完整流程仍未验证。

## Windows性能结果

- 10k真实合成文件、100目录：初扫19.256s，不变/单修改重扫4.525/4.819s。首次非空结果约95.5ms，期间并发查询P95 30.410/12.001/11.651ms；全映射、事件、收藏和关系核对通过。见[扫描报告](validation/2026-11-21-windows-scan-report.json)。不等于原生可见耗时P95。
- 100k合成混合实体：搜索996页99,599项；时间线976页97,533项；2,240次独立核对零错。名称搜索全链P95 11.010ms，深度检查均未超60ms。时间线全链P95 15.871ms，首次日历未命中1616.734ms单列，不能称首次月历低延迟。
- 见[完整分页报告](validation/2026-11-21-windows-pagination-report.json)。结果仅限该OS/CPU和既定合成模型，不证明冷磁盘、完整原生P95、低端机或其他排序的Windows全面性能通过。

## 剩余完成条件

- **核心失败：** 本机WPS 12.1.0.29690安全保存后File ID变化且无既存Object ID，原引用不能自动延续。需要足够的原生文档连续性证据或明确批准的产品/架构方案，不能以同路径、创建时间、手工修复或修改原件元数据替代U8通过。Word及跨日保存未执行。
- **未完成原生分支：** SQLite实际运行中断、动态其他组合及其他DPI。日期同日包含、次日排除和无效区间恢复已有末尾补验证据；其余未测边界不扩大登记通过。
- **已补齐：** 三类同批外拖及取消/重试、内部单项/多选关联、重复及非目标取消、默认网页打开、动态B的修改/发现/打开筛选、交付EXE正常重启取回、记事本运行中/关闭期间保存、110项原生分页全遍历、普通C:\路径筛选及中英最小窗口的已记录部分。依据见下方补验段及日志。
- **后置边界：** 发布签名/安装升级、完整覆盖恢复和用户试用保持原决定，不扩大本轮范围。Mac历史缺口不由Windows结果补记为通过。

## 后续原生补验（同日）

- 三类同批Explorer外拖：第6份为目标重复、第8份为已授权未关联、第9份未授权。实际回执1加入/1已存在/1未授权/0失败；A由6变7。取消原生授权后A仍7且第8份保留；重试授权后A变8，只授权第9份。见three-way-drop/cancel/authorized日志。
- 原生应用内拖第9份至B，B由3变4，SQLite确认第9份A/B双关系，未移除来源。见internal-drag日志。
- 正常退出release进程28660，直接运行交付目录EXE得到新进程27944；A8/B4保留，从B打开第9份进入记事本，原生控件正文为MIXED_UNAUTHORIZED_FIXTURE_9。见restart-workspace/default-open-restart日志。TXT编辑自动键盘输入未取得落盘标记，不计保存通过。
- 动态实际跨11月→10月→11月；撤权后当月4条仅为独立授权范围内记录。英文900×640最小窗口显示图标侧栏、常驻搜索、月历和可滚动时间线，见windows-en-minimum.png和native-en-minimum.log；不是其他DPI全面验收。
- 先前手势失败仍保留。至此三类混合外拖及引用重启取回取得证据，剩余内部多选/取消与其他未覆盖分支仍按实际记录；WPS U8仍明确失败，完整V1结论不变。
### 记事本保存补验

通过原生Notepad编辑控件（WM_SETTEXT）输入合成标记，并向记事本发送正式保存命令（WM_COMMAND/Save），而非直接改写文件。运行中保存与WorkTwill正常退出期间保存的标记均落盘；候选再次启动为PID30072。第9份TXT始终是UUID f7fcc275-ba02-4f60-a0f3-bf3f29e95a64、同File ID/creation time、available、A/B双引用且仅一条资源。关闭记事本后，从重启后的B重新打开，原生控件正文包含两个保存标记。见notepad-running.json、notepad-after-restart.json、notepad-reopened-body.log。此前SendKeys未落盘的尝试保留为失败操作，不与本次原生控件保存混淆。此结果限记事本同物理身份TXT；WPS安全替换、跨日仍未通过。
### 原生完整搜索路径补验

Ctrl+K后焦点实际为全局搜索ComboBox，输入“混合”返回第8/9份。点击完整结果后输入普通C:\绝对路径到第9份文件名前缀，实际仅余第9份；输入不存在路径显示“没有匹配的资料”，清空后可返回原B。见native-search-path/empty日志。此覆盖路径修复的正式原生入口，不替代100项以上原生分页、日期全部边界或SQLite实际中断证据。
### 原生110项分页补验

另建独占native-pagination目录和110份合成TXT，经原生选择器授权；全局搜“分页验收”进入正式完整结果。首批100项滚至099，Invoke“加载更多”后可访问树保留094–099并追加100–105，没有回顶；滚到底显示100–109且无继续加载按钮。按0–100%分11个滚动位置采集正式虚拟列表，观察到110个不同文件名、缺失0。见native-pagination-append.log与native-pagination-all.json。核验后在设置撤销该独占目录授权；没有清空目录或改变用户其他授权。此为原生分页/虚拟滚动正确性证据，不是原生P95或SQLite中断证据。
### 内部多选、重复和非目标松手补验

在A选中第6/8份，真实鼠标拖第8份至B；B4→6，逐个UUID核对两份均A/B双关系。再次选中同两份拖到B，B仍6，活动计数149不增加。随后第8份拖至页面标题的非目标区域松手，A8/B6、总关联14、活动149均保持。见internal-multi.log、internal-multi-state.json、internal-repeat.log、internal-cancel-state.json。没有把UI“2项成功”文案等同于新建两条关系，重复分支按数据库无变化判定。
### 默认网页打开补验

从A点击“打开 Windows 验收网页 Example”，默认Chrome窗口标题实际为Example Domain - Google Chrome，地址栏Value为example.com。见windows-default-browser.log。未读取用户浏览器其他标签或登录资料。
### 动态按工作区和事件类型补验

正式动态选择工作区B。文件修改Toggle筛选→日历/当月2条且明细为第9份TXT的运行中与关闭期间保存；首次发现→3条，对应仍有效的第6/8/9份独立授权文件；应用内打开→2条且第9份真实打开记录。撤销目录的其他资料未显示。选择11月20日（没有对应活动）后显示该日标题，返回整月。见timeline-modified-B/discovery-B/opened-B/empty-day日志。未人为改系统时间制造跨日Office通过。
### 原件最终复核纠正

最终复核发现第1份TXT在05:38:39Z被早前Notepad SendKeys尝试写入 `INDOWS_NOTEPAD_SAVE_RUNNING` 前缀（焦点误中另一合成文档）。早先各时点5份均不变的观察只对当时成立，不能作为全程不变结论；最新source-integrity.json如实记录第1份hash/size/mtime变化，第2–5份仍全部不变。此为验收外部编辑器操作造成的合成样本修改，不是WorkTwill写入的证明；保留原baseline和现文件，不回写基线或恢复掩盖。第9份的WM_SETTEXT/Save闭环有明确目标句柄及落盘证据，独立成立。后续只用未编辑第2–5份核验引用操作安全。
### 日期筛选及错误恢复修复

原生键盘输入并读回日期分段：2026-11-21至21包含第8/9份；改为22至22无匹配。初次错误方法ValuePattern.SetValue未实际提交（读回空），不计日期通过。合法查询曾残留编辑期间无效范围错误，已修App：仅成功列表查询清除source=page的旧错误，其他操作错误保留。新增回归验证日期修正消除错误、收藏失败不被查询吞掉；定向100项通过。新release原生复验：22至21报错，改22至22清旧错且空，改21至21恢复两项，见date-fix-invalid/valid-empty/inclusive日志。

旧完整检查在时间线迟到游标用例两次5000ms超时，原始日志保留为check-date-fix-timeout及timeout-second；首轮存在编译/测试竞争，第二轮仍失败。Windows的vitest文件改为串行执行，保留默认超时和全部100项分页/旧响应断言，不提高门槛、不删测试；最终结果以check-date-fix.log为准。

新候选：WorkTwill-v0.1.0-windows-x64-candidate-date-fix.exe，SHA256 `1B8BB529835767D4EF33828F2EC3A51CE8919C6D98255F895E023884353BCF33`。直接启动交付文件PID30596，窗口响应。旧候选保留历史指纹；WPS连续性仍失败，未作更改。
### 当前交付核验

最终串行完整检查退出码0：5个前端测试文件共144项、Rust183项通过，5个显式规模用例默认忽略；DTO、生产前端构建、fmt及严格clippy通过。ZIP SHA256 `37A233B9796F674EDD64784F37CDA6785A5232DE6C51E48D56C4805A6F998D9D`，只读读取ZIP内EXE并计算摘要，与交付EXE完全一致。未提交、未推送；完整V1结论仍为未通过。

### 收尾远端复核

初次同步已成功快进到 e055f72c12ae5d2bb762e08dc1cec845b59953b7。收尾再次 git fetch origin 时，GitHub HTTPS 凭据不可用（could not read Username；/dev/tty unavailable），未能再次确认远端是否新增提交。当前 HEAD 与本地 origin/main 均为上述提交；不能把本地引用相同写成收尾时已确认远端最新。此操作未提交或推送代码。U8阻塞的约束与待决策选项见 [独立记录](WINDOWS_U8_BLOCKER.md)。

### 英文核心状态补验（当前候选，无源码变更）

直接操作交付日期修复候选，英文首页、设置和工作区均取得UIA记录：权限撤销位置明确标记“Authorization revoked; references retained”，3个独立授权文件可见；工作区未授权条目显示“Unauthorized file”匿名占位，未显示旧名称/路径。无匹配搜索显示“No matching resources”；先前未前置窗口的Esc尝试没有关闭下拉，保留search-escape.log，不登记通过。随后明确前置应用、聚焦Global search并按Esc，下拉消失且动态上下文保留，见search-escape-corrected.log。这里只证明界面取消，不证明SQLite实际执行中断。

英文动态Modified筛选显示当月2条、明细均File modified；Previous month转October 2026显示0 activities、No activity for this month及No resources in this date range。随后返回11月并恢复中文首页。证据为同日期windows-en-followup-home/settings/workspace/search-empty/search-escape-corrected/activity/modified/empty-month.log。原生语言切换是异步，首次切中文后立即按中文名称查控件未找到，短暂等候后成功；未据此登记产品缺陷。未执行其他DPI矩阵。

本轮只复核未被外部编辑的第2–5份原件，hash/size/mtime全部与原baseline一致，见windows-en-followup-integrity.json；不覆盖第1份历史误编辑记录。无运行时代码变更，未重复构建或完整检查。WPS U8失败和实际SQLite中断证据缺口不变。

### U3 单条移除及其他关系保留补验

原生A选第8份，点击收藏后选择状态被重读清除，首次“从本工作区移除”查找失败且未执行；重新选中后正式移除成功，A8→7、B仍6。原生B仍显示第8份及“取消收藏”按钮。只读SQLite确认同UUID 09b94987-de2f-468b-baab-e012df82a711，available、favorite=1，仅B关联。原件路径存在，移除前后SHA256、大小、mtime完全一致。见unlink-before/after.json、unlink-source.log、unlink-other-workspace.log、unlink-source-integrity.json。此为U3明确单条移除闭环；没有将其他U8或跨目录5文件场景一并登记通过。当前最终状态A7/B6（此前A8/B6为当时真实状态）。

### 完成审计与WPS阻塞复核

逐项核对目标与PRD后，结论见[完成审计](WINDOWS_V1_COMPLETION_AUDIT.md)。本轮只读fsutil核对失败DOCX，File ID仍为0x0000000000000000001500000002182c，与原正式失败快照的新对象一致；仍无Object ID，见windows-wps-blocker-recheck.json。没有改变身份或授权代码，没有重复保存、构建或检查。完整Windows V1仍未通过；已通过的外围流程不能抵消U8核心失败，未验证项继续保留。

### 跨目录5文件收纳及重启取回补验（U2部分）

独立u2-cross-directory夹具新建5目录各1TXT，保存基线。新建“U2 跨目录客户验收”，原生选择器通过文件名输入框WM_SETTEXT及正式Open逐份加入；这不是搜索结果一次多选，不能登记完整U2精确步骤通过。首次ValuePattern拒绝写入、SendKeys读回不一致、多路径提交未导入，以及窗口未就绪查控件失败均未计成功。正式单路径逐项读回后提交，工作区显示5项。

交付日期修复EXE正常CloseMainWindow退出PID30596，直接重启为PID33288。从重启后的工作区看到5项，前后只读SQLite核对全部UUID/路径/状态/关联完全一致；点击第5份默认打开Notepad，目标原生编辑控件AutomationId15的Name严格等于WORKTWILL_U2_CROSS_DIRECTORY_5（该控件不支持ValuePattern，失败读取未计成功）。5份源文件hash/size/mtime全部与独立基线一致。见u2-before/after-restart.json、u2-cross-directory-before-restart.log、u2-restarted-workspace.log和u2-cross-directory-result.json。未编辑原件；没有改动运行时代码或构建。WPS待决策问题仍未获批准，不创建Object ID。

### U2 搜索多选5份、新建并加入、重启打开补齐

全局搜索“U2客户验收”实际返回5个不同目录中的文件，进入完整结果，依次Toggle选择5份，点击“新建工作区并加入”，命名“U2 搜索多选客户”，一次提交后显示5项。正常关闭交付候选PID33288，再直接运行相同EXE得到PID30968；点击该工作区仍为5项。前后只读SQLite核对5个唯一UUID及路径/状态/关联完全一致。点击第4份默认打开Notepad，原生编辑控件Name为唯一正文WORKTWILL_U2_CROSS_DIRECTORY_4。5份hash/size/mtime与独立初始基线均一致。见u2-search-selected/created/restarted.log、u2-search-before/after-restart.json及u2-search-result.json。至此U2搜索多选、新建并加入与重启取回组合已实际执行；此前逐份选择器补验不再是此精确步骤的唯一证据。不扩展为U8的Office连续保存/跨日通过。无运行时代码变更，不重复构建或完整检查。

### Windows网页标题关闭与失败手填补验

当前候选实际首页打开“添加网页”，取消“自动获取网站标题”，输入https://example.com/worktwill-manual-off及标题“Windows 标题关闭手填”，保存后列表可见。再次添加https://worktwill-acceptance.invalid/，自动标题返回“仅自动获取公网网页标题，其他网址可手动填写”，手填“Windows 标题失败手填”后保存成功。只读SQLite确认两条kind=web、标题/URL与输入一致，workspace=null；此次实际在首页，属于全局资源，不能声称已加入工作区。

证据为web-title-off-form/saved.log、web-title-failed-form/saved.log、web-title-saved.json。自动标题失败是保留.invalid域名的策略拒绝，不代表实际网络超时/离线分支通过；关闭开关时未做网络抓包，不声称已从系统层证明零请求。与先前example.com自动标题成功/可修改共同覆盖这些实际分支。无代码变更，不重复构建或完整检查；U8仍失败。

### 网页类型筛选及仅移除一条关联补验

首页选中“Windows 标题关闭手填”，通过原生加入工作区表单依次关联“U2 搜索多选客户”与“U2 跨目录客户验收”。首次按名称查目标工作区误取Text标签，SetFocus报无法接收焦点；后续明确ComboBox/SelectionItem操作完成，数据库验证两关系后才登记。两工作区各6项（5文件1网页）。进入前者点击“网页”，仅显示该网页；选中后“从本工作区移除”，前者回5项且网页筛选不再显示它。进入后者点击“网页”，该网页仍在。

只读前后快照核对同一UUID 0e86d36d-139d-4740-bf0a-7cab21e18c10、标题和https://example.com/worktwill-manual-off完全不变，只剩“U2 跨目录客户验收”关系。见web-unlink-before/after.json、web-filter-before/after-unlink.log、web-other-workspace.log。此覆盖网页筛选和单关联移除保留另一工作区的分支；默认浏览器打开example.com沿用此前真实证据，不声称此次合成子路径实际页面可访问。无运行时代码变更，未重复构建和完整检查。WPS核心失败与其他未验证项不变。

### 独立搜索性能补验及Windows夹具修正

首次显式执行两项搜索规模用例，未规范化的合成目录路径与正式Windows授权入库的\\?\前缀不一致，10k中文首查询为空；混合用例242核对中209失败。保留search-performance-path-failed.log及search-mixed-path-failed.json，不记为产品性能通过。只在acceptance_search.rs与acceptance_search_mixed.rs将已创建的根目录canonicalize，与正式授权一致；权限外目录仍独立并显式撤权，不改变权限谓词和断言。

修后cargo test --release -p worktwill-core disk_search_acceptance -- --ignored --nocapture --test-threads=1退出0，两项通过。独立文件搜索分别10k/50k/100k/500k，每组165次独立结果核对；100k并发收藏写入94次、查询核对150次。后端P50/P95/P99(ms)：10k 15.001/20.623/23.613；50k 6.885/16.453/19.694；100k 8.053/23.213/30.048；100k并发9.039/23.584/26.117；500k 16.165/83.807/92.274。500k压力P95高于60ms，测试原有预算只约束100k，不扩展为500k达标。

100k混合实体22类查询、220热样本、242次独立核对零错，总体P50/P95/P99为6.927/29.997/36.181ms，各类最慢P95为45.665ms，budget_failed=false。见search-report.json、search-mixed-report.json与search-performance.log。首次各类查询作为预热排除；不是冷磁盘或原生输入到可见时间。内存约15.79GiB，C盘NTFS；其他OS/CPU见环境段。新增夹具修改通过cargo fmt --all -- --check、core all-targets严格clippy，后者见search-fixture-clippy.log。未修改产品运行时代码，不重新构建候选；完整工程检查沿用date-fix日志，新增规模测试结果单列，不将183改记185以混淆默认执行数。原生可见P95、SQLite原生中断及WPS U8仍未关闭。
