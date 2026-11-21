# Windows V1 工程验收 · 2026-11-21

结论：**正式连续性核心、设置入口和新Windows候选已实现，真实WPS运行中/退出期间保存恢复取得通过证据；完整 Windows V1 仍未通过。** 仅限明确启用的NTFS目录追踪，竞争UUID继续拒绝，不扩大为所有保存方式或全部U8通过。

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

### 保存连续性隔离PoC第一轮

用户批准按多agent审查方案尝试，已在全新合成目录进行无Object ID/创建Object ID对照及无关替换实验，见[PoC实测结果](WINDOWS_SAVE_CONTINUITY_POC_RESULTS.md)。创建标记组的WPS连续保存保留Object ID，但File.Replace无关DOCX覆盖也保留该Object ID；不能单独作为自动业务关系继承依据。没有修改正式身份策略、生产数据库或默认原件只读规则。Object ID实验元数据写入仅限新建合成文件；U8及五项验收清单未提升为通过。

### 应用层隔离绑定PoC及长期架构结论

新增独立example continuity_poc，以真实Store关系和查询验证候选绑定策略。WPS保存后实验保留UUID/双区/收藏/单资源且resolve成功，但recent为空；复制独立，无关ReplaceFile也被标记策略接受，撤权后访问拒绝并隐藏收藏。未修改生产绑定流程。明确SQL模拟不等于正式自动恢复；真实GUI、单文件授权、并发及历史权限尚未通过。方案与证据边界见[长期架构结论](WINDOWS_DOCUMENT_CONTINUITY_ARCHITECTURE.md)。构建、fmt与example严格clippy通过；不重建候选，不提升U8结果。

### 用户批准正式约束及风险定向验证

用户明确同意四层架构、明确替换的逻辑连续性及显式追踪同意后按需创建Object ID，并追加六项实现约束。PRD §4.3与ARCHITECTURE Windows章节已同步；不再因同一已批准产品取舍阻塞。在线事件不是离线保存恢复的前置，当前访问、连续性及历史展示分别验证。

独立WPS COM连续12次保存与300次扫描，最终14条文件资源，判定reject_existing_resource；收录锁文件及多个contract.docx身份，未抓到.tmp替换源先入库，特定强制时序仍未完成，不新增静默合并规则。见wps-scan-race.jsonl及result.json。混合授权负例：先单文件授权后目录授权，WPS保存后仅SQL更换identity，撤销目录后旧grant仍active=1、收藏及resolve仍可访问。该负向结果确认必须在正式续接事务撤旧grant；不是产品修复通过。见mixed-grant-naive-apply/revoke/state.json。例程序新增独立扫描竞争/混合授权初始化，构建fmt及严格clippy通过；没有接入正式自动续接，U8仍未通过。

### 扫描时序与混合授权定向验证

见[定向结果](WINDOWS_CONTINUITY_TARGETED_VALIDATION.md)。新增同路径替换的先单文件后目录授权回归通过，完整relink_grants 9项通过；这是人工事务基线，不是自动续接完成。强制.tmp先扫描再File.Replace两组：普通创建时间组候选策略接受但仍留2资源；创建时间一致控制组reject_existing_resource。二者均为受控替换，不冒充真实WPS在.tmp阶段暂停后的复现。当前identity含创建时间，说明冲突判定不能仅靠一次identity精确匹配。未增加静默合并规则，正式自动服务与U8仍未完成。

### 正式连续性核心服务首版

按用户确认约束实现windows_snapshot/document_continuity、schema9、独立历史链recent解析及统一扫描/通知/导入/原路径打开复核入口。4项新正式入口测试通过，真实WPS COM连续3次保存通过正式扫描自动保留正文UUID/双区/收藏/recent，锁文件独立记录如实单列。完整pnpm check退出0，UI/API144项、Rust188项通过，5规模默认忽略，DTO/前端build/fmt/clippy通过。见continuity-full-check.log及[实现边界](WINDOWS_CONTINUITY_SERVICE_IMPLEMENTATION.md)。未接入设置UI、逐文件结果与自动锚点建立，未构建新原生候选，U8仍未通过。

### 正式设置与新原生候选闭环（2026-11-21）

本轮有代码修改：schema10逐文件失败、DTO/IPC/权限、中英文明确同意及结果展示；收藏/工作区关系与原生导入成功后维护锚点，维护失败单独报告，不伪装关系提交失败。145 UI/API通过，workspace Rust190项通过、5规模默认忽略；最后新增复制独立/同目录改名测试后，连续性7项全部通过、严格clippy再次通过。DTO/build/fmt通过。完整检查中间类型推断失败及开发中fmt失败日志保留，最终Rust见continuity-ui-rust-final.log，定向见continuity-final-regression.log。

新候选[连续性EXE](../deliverables/WorkTwill-v0.1.0-windows-x64-candidate-continuity.exe)与[ZIP](../deliverables/WorkTwill-v0.1.0-windows-x64-candidate-continuity.zip)，指纹见[清单](validation/2026-11-21-windows-continuity-candidate.json)。此前日期修复候选不含这些功能。新候选从schema8正常启动迁移到10；PID30768正常关闭，直接启动同一制品PID39656。无签名/安装器。

新合成DOCX起初无Object ID；正式目录授权、收藏后仍无标记。设置启用进入明确同意，取消不写标记；同意后Object ID建立，UI逐文件显示成功。原UUID79c9737a-570c-4301-8e37-56961f3c1fb5收藏并加入A，从正式入口打开WPS。真实Ctrl+S保存1、保存2和应用关闭期间保存3，ZIP正文独立只读核对三个标记均存在。重启扫描自动更新绑定，原UUID/收藏/A保留；最近使用可见并可再次打开。GUI/COM区别保留：本三次来自键盘Ctrl+S，不用先前COM结果替代。

随后加入B，第一次及后续按键因WPS未获焦点未写入，均不登记成功。COM附加尝试等待未返回后中止，不作保存证据。实际提升WPS窗口、正文坐标点击取得焦点后第四次Ctrl+S写入标记成功，绑定版本3，双工作区2/收藏1/正文UUID不变。四次保存对应三段绑定历史（一次通知合并），不声称逐次事件必达。首次ZIP独占读被WPS锁拒绝，改为共享只读验证，不修改源文件。

中英设置显示建立结果；关闭追踪后仍保留既有Object ID。重新启用后撤销该目录，recent不再显示文档；只读库active=0、无单文件grant、关系仍2。此原生夹具没有旧单文件grant，混合授权安全依靠正式入口回归单列，不冒充原生混合授权已测。见native-tracking-enabled/english.log、native-continuity-before/save-1/restarted/body/recent/dual-save-4-final/revoked.json及revoked-ui.json证据。

当前U8只取得目标WPS/NTFS显式追踪子集通过；跨日、Microsoft Word、真实WPS临时源先入库精确顺序仍未完成，强制竞争测试保持拒绝并需确认，不静默合并。原生SQLite中断、完整输入到可见性能与其余DPI矩阵仍缺证据。不能宣布完整V1完成；这些是剩余工程验收，不需要用户重复确认既有产品规则。

最终候选补修逐文件状态的当前身份复验，冲突时不再仅凭库内锚点显示成功。7项定向回归含冲突状态断言通过，严格clippy通过；重新release构建、替换EXE/ZIP并正常启动设置页。最终EXE SHA256 D9E4E7B00D4E93149D4945A64DEAF0234714BF78C16CB87A45985D3B82ED48FA，ZIP 9CDC8B86DD945D6D06524776EA29F676AC6D26841AD8D823904B242059CCDE24。上面的真实保存来自8EDDA27D构建，最终构建只补状态复验，未重新登记四次原生保存。见status-revalidation/status-assertions/status-build和final-candidate-smoke日志。

最终D9E4E7B0候选再次通过原生选择器重新授权同一合成目录，设置页实际读取当前对象后显示标记已建立；recent原UUID可见并从原入口再次打开WPS。见final-tracked-status/final-recent.log。该补验确认最终状态复验正常分支，不冒充重跑全部保存场景。

### 目标续行：当前Object ID复验与最终构建保存（2026-11-21）

HEAD仍9124cc6，未提交改动全部保留。当前关口按[剩余关口](WINDOWS_V1_REMAINING_GATES.md)逐项核对，不以旧“待批准”说明阻塞已批准方案。发现同物理文件Object ID被外部移除时逐文件状态仍可能虚报成功；正式状态读取现在核对卷、当前Object ID及已绑定身份。新增独立TempDir反例验证可打开/收藏保留但追踪明确失败；不删除失败检查或改变授权语义。

完整`pnpm check --fixtures C:\Users\Administrator\AppData\Local\Temp\worktwill-windows-v1-20261121`退出0：145 UI/API、192 Rust（含原生壳22项），5规模默认忽略；DTO、生产build、fmt、严格clippy通过。新增后连续性8项包含在该轮192中，不另行累计。日志marker-full-check及marker-revalidation保留。早先口头误报200已按日志逐套求和纠正。

新release构建成功并保留旧制品，新EXE `WorkTwill-v0.1.0-windows-x64-candidate-marker-check.exe` SHA256 `C6D3F90574ACB7646BD48C25B26A8C94DE942037DDDCB8DA80C08055FA523D6B`，ZIP `0E1C5A4B6FE77118F2498879C20C80684DCEC4DBFE36CE7A04F75308E955D0C9`。直接启动PID40828，设置页当前Object ID复验成功。

该构建保存前已有A/B双区及收藏。WPS真实键盘Ctrl+S保存save1、save2，又重复save1；正常关闭WorkTwill后保存offline3，直接重启同交付EXE PID33872。独立共享只读DOCX XML确认三种标记均存在；WPS自动调整大小写，早先区分大小写的false记录保留，纠正记录为marker-u8-body-running-corrected和body-restarted。数据库保持原UUID79c9737a、双关系、收藏、available，绑定版本3→7，没有同名重复资源。不能把WPS自动大小写的核验误判写成产品未保存。

重启后分别Invoke A/B/收藏正式打开入口。B与收藏分别先在WPS正文Ctrl+W，读回窗口标题WPS Office，再从入口重新打开，截图body-B-loaded及body-favorites明确显示offline3和此前保存标记。A入口已调用且任务栏提升后观察正文，但初次Ctrl+W未独立确认关闭，因此A的冷重开仍需补齐。截图reopened-A/body-A实际为被遮挡界面，不能登记正文通过。原生UI启动未就绪的两次控件查找失败保留，不算产品失败或成功。详见marker-candidate.json及marker-u8-*证据。

完整Windows V1仍未通过：临时源竞争、真实跨日、该构建副本/另存和撤权补验、SQLite实际中断、完整可见性能与其余DPI等继续推进，不以本轮保存子集或构建通过替代。

A冷重开随后补齐：普通SetForegroundWindow/鼠标多次未取得焦点，均未当成功。使用AttachThreadInput连接前台线程后Ctrl+W，读回WPS Office；再次从A正式入口打开，将WorkTwill最小化后截图`marker-u8-body-A-reopened.png`显示正确路径、save1/save2/offline3正文。至此本构建三入口关闭文档后重开取得同日证据，跨日及其他剩余关口不变。

### 当前候选复制、撤权和真实临时源观察

C6D3F905运行中，将合成原件复制为`独立复制验证-3f8708e1.docx`，复制品无Object ID；正式扫描建立独立UUID349a021f，favorite=0、工作区关系0，原件UUID/收藏/双关系不变。见marker-copy-independent.json。此为真实文件系统复制+正式原生扫描，不等同WPS GUI另存为。

从正式设置撤销合成目录，收藏无原件名称，工作区匿名占位，搜索无原件打开结果；UIA日志marker-revoked-favorites/A/search保存。fsutil确认原Object ID仍2f821945…；只读库保留双关系和收藏，夹具无旧单文件授权，不扩展为混合授权原生通过。随后经原生目录选择器重新授权同目录，追踪状态正常；误选无标题辅助窗口造成控件缺失已定位，未计产品失败。

真实WPS临时源受控观察新增两组结果，见[实验边界](WINDOWS_WPS_SOURCE_CAPTURE.md)：不共享删除的临时源先入库已捕获，但干预导致Object ID不保留；挂起WPS后扫描尚未收录临时源。不能据此宣称已关闭正常保存竞争，也未修改既有UUID冲突保护。WPS GUI另存为尝试尚未定位到当前文档对话框，不登记成功。

### 锚点建立事务提交前复验

继续源码复查，在establish_document_anchor写入数据库后、commit前增加完整快照复验；文件系统对象或标记变化时回滚数据库结果，创建的系统标记不伪称可回滚。这是现有TOCTOU/失败回滚契约补齐，不增加文档身份推断或冲突消解规则。8项连续性回归及workspace严格clippy通过；未无目的重跑前端或规模套件。release构建及git diff --check通过（Git换行提示保留）。

新候选anchor-commit EXE SHA256 `2B5885541A80688659C02049C8B9E1E18432DBFAE5B4B4C436E95F06F62C4259`，ZIP `DF61033ED554D0C7733265FD2C5FEFAE9FF9BAE5AD31406ACD9EBF9C3A1FD912`。通过原生WindowPattern正常关闭C6候选，直接启动交付新EXE PID42748，设置允许追踪及标记建立正常。该最终构建尚未重跑U8，C6保存/复制/撤权证据不冒充2B构建证据。清单为anchor-commit-candidate.json，日志anchor-commit-recheck/native-build/native-smoke。

18:22观察桌面正在执行另一交互任务，暂停鼠标键盘试验以免干扰，继续源码、测试、构建及UIA只读/Invoke验证。没有宣称无法原生操控或需要用户重复授权。最后已重授权原合成目录，原引用恢复。跨日、目标编辑器/目录矩阵、正常WPS临时源先入库、GUI另存为、SQLite实际中断、原生可见性能及DPI关口仍不通过。

### Windows身份同句柄采样及并发/历史边界

正式platform::identity此前从当前句柄读File ID，却拼接调用方旧metadata的创建时间；路径在两次读取间替换可能得到不存在的组合。现从同一句柄读FileIdInfo/FileBasicInfo，字符串格式保持不变，不需要数据迁移。新增独立TempDir反例强制创建时间差异，旧metadata传入也必须返回当前同句柄身份。该反例通过；完整identity-full-check退出0：145 UI/API、194 Rust（含壳），5规模默认忽略，DTO/build/fmt/clippy通过。

新增四线程同时续接仅一次提交、版本/历史各增1、原观察身份保持、旧file grant失效及recent/收藏保留；随后新增当前路径仍授权而旧观察路径被排除时recent隐藏、当前打开/收藏仍有效。连续性定向10项通过；后一个测试在完整194之后单列，不累计冒充同轮套件。

对应1D8D3668候选release构建并启动交付文件PID37452，读取既有授权/锚点正常；完整保存未重跑，清单identity-candidate.json。此后发现暂停/离线目录无法关闭追踪同意，核心与UI已修：关闭不要求目录可访问，UI只限制暂停目录启用、不限制关闭；保留锚点。新增核心与UI回归通过（连续性11项、UI2项），完整检查与新release正在执行，以后续日志为准，不提前登记通过。

### 暂停目录撤销追踪同意原生复验

新consent-stop候选EXE `DE1026FE7B43F2CD971F6596D47229997E1D640C88B5BD4305943B3D6D4B23E7`、ZIP `2A84BCF93E4EBB2163D0EE4FBBCB0E4067EED270A436DD449D1BE9102965F24D`，原生启动PID29936。正式设置暂停扫描后，“关闭追踪”IsEnabled=true，点击后UI未启用、数据库paused=1/enabled=0、原Object ID仍2f821945…，既有锚点保留。随后继续扫描并经同意入口启用，状态复验成功。证据consent-paused-button.json、paused-db.json、paused-disabled.log、native-restored.log。该构建尚未重跑保存U8。

consent-full-check首轮因时间线“旧续页晚到后仍使用新筛选的游标继续加载”5000ms超时而失败，其余145 UI通过；当时release编译并行，原始失败保留。编译结束后原命令重跑，不修改超时或断言，以consent-full-check-retry.log最终结果为准。release成功不能覆盖检查失败。

最终重跑退出0：146 UI/API、196 Rust（含壳22项），5规模默认忽略；DTO、build、fmt、严格clippy通过。连续性11项包含于本轮196，不重复累计。新候选清单consent-candidate.json。900×640原生物理窗口读回尺寸一致，GetDpiForWindow=120（125%），中英文设置均可读取常驻搜索、追踪同意、逐文件结果；见consent-en/zh-minimum.log及minimum-geometry.json。此为设置页/DPI组合，不冒充所有窗口流程或其他DPI通过，操作后恢复中文1280×900。完整U8仍须在DE构建补验，不能借C6结果登记最终构建通过。

继续复查离线分支：tracking_root此前目录metadata失败会使整个document_tracking_status失败，UI隐藏全部设置入口。已改为不可读取目录不提供追踪资格但继续返回同意状态，允许关闭；没有据路径自动重授权。新增未暂停目录离线时状态可读/同意可关闭回归，连续性12项及严格clippy通过。此变更晚于DE构建，当前源码新release构建中；DE不含该修复，不能作为最终源码制品。日志tracking-offline-status和offline-status-native-build分别记录，未提前登记构建或原生验证通过。

最终离线状态修复后workspace197项Rust通过、5规模默认忽略，严格clippy通过；UI源码未变，复用此前完整146 UI/API。release成功，新offline-status候选EXE SHA256 `58BF5B34A9405913ED7C391B6D2FC11EEF13ABF846CB9D107B5EB1821BB6904B`、ZIP `C2F2BE944586FFEC16941BD7F7A9A9BEE4EE44CAF7902A4746C03D94C5C068BD`，ZIP内EXE摘要核对一致。正常关闭DE后直接启动交付EXE PID39464，设置当前标记建立及既有状态正常。见offline-status-candidate.json、native-smoke/state。原生离线目录及完整U8尚未重跑，不把核心回归或旧候选证据扩大为该构建通过。

### 58BF5B34最终构建同日U8、GUI另存为与撤权

同一最终EXE重新启动PID44840后，保存前原资源已在A/B并收藏。正式A打开WPS，Ctrl+S两次写入FINAL58RUNNINGONE/TWO，独立只读ZIP XML均命中。正常关闭WorkTwill，再在WPS写入FINAL58OFFLINETHREE并Ctrl+S，直接启动同交付EXE PID39308，三标记均落盘。库内原UUID79c9737a、available、favorite1、links2且同名仅一条，绑定版本7→10。见58-u8-running/body/offline-body/restarted。

B、收藏及随后A分别先Ctrl+W关闭目标文档，WPS标题变为另一合成contract.docx，再从正式入口重开；截图58-u8-B/favorites/A-reopened均显示三个新标记。A首次关闭仍为原文档标题，保留A-closed.log不计冷重开；后续重试A-retry-closed.log确认关闭，补验成功。B第一次入口过早读取未找到按钮，等待加载后成功；自动化主窗口辅助脚本改按进程+WorkTwill标题枚举，避免误用无标题辅助窗口，不修改产品源码。

真实WPS GUI F12的另存为是嵌套子窗口659274（不是其他进程遗留68980）。ValuePattern写入无效读回原名，未保存；随后明确焦点、点击文件名编辑框，键盘输入新合成路径并读回完全一致后Enter。新`另存为独立验证58.docx`实际存在，当前候选扫描建立UUIDd9597cfe，favorite0/links0，无Object ID；原件UUID/identity/favorite1/links2保持。两份DOCX正文均含三个标记，见58-saveas-keyboard-value/state/body。仅此具体WPS版本/目录组合，不推广Word或其他文件格式。

从正式设置撤销目录后，收藏/工作区旧名称泄漏0，搜索打开结果泄漏0，工作区保留匿名占位；原Object ID仍2f821945…，数据库双关系和收藏保留。见58-revoked-*。无旧file grant的原生夹具不冒充混合授权通过。撤权后当前目录保持撤销，待后续重授权补流程。

离线原生实验两次目录移动被Windows占用拒绝，均没有移动成功；未登记离线通过。另建TXT夹具选择器未接受输入，未增加授权，保留夹具不清空；最终58构建离线安全当前仍依靠12项核心回归，原生证据未完成。真实跨日、正常临时源竞争、SQLite运行中断、完整可见性能与剩余DPI关口仍不通过。

### 当前构建原生输入至可访问结果测量

58构建、现有小库、原生UIA ValuePattern写入全局搜索，从SetValue开始计时，轮询正式search-result-0且名称必须以U2客户验收_1.txt开始；每轮先输入不同无匹配词，等待100ms，再输入目标词。30次全部观察到对应结果，P50 100.4482ms/P95 116.4213ms/P99 119.694ms。测量包含UIA轮询和IPC观察开销，观察的是可访问结果出现，未单独验证像素呈现时刻。见58-search-visible-corrected.json及report.json。

首次探针错误按精确名称U2客户验收_5.txt查找，结果实际名称含路径与“打开”，超时3秒但UIA树已存在结果；保留visible-samples及timing-missing日志，不作为产品慢查询样本。改用正式结果ID后才取得上述30次结果。当前数据规模远小于100k，不证明PRD100k输入到可见P95≤150ms，也不证明SQLite运行中断；规模原生关口继续未通过。当前原合成目录仍撤权，8个独立授权文件可用，未向正式库注入规模合成SQL。

撤权后再从所有文件→最近使用以及动态取UIA树，原件名称泄漏均0，见58-revoked-recent/timeline及history-assertions.json；工作区/收藏/搜索的先前结论保持。原生新目录选择再次输入读回空，未提交授权，关闭该选择器，未改变业务库。源码和58候选均未变化，因此未重复构建或无关全套检查。

### 连续性冲突隔离与四入口负向验证

新增正式扫描/通知/导入/打开四入口：先扫描替换源B再ReplaceFile覆盖A，所有入口保留A/B UUID和identity，不写绑定历史、不转移关系；同名无锚点重建保持独立并拒绝旧引用打开。首轮scan_all误期望成功，实际混合授权复验明确返回continuity_conflict；按既定要求纠正为断言该错误，原失败entry-conflicts.log保留，后续entry-conflicts-retry通过。

反例进一步发现带旧单文件grant冲突会使全量扫描在文件授权复验阶段退出、饿死其他目录。修复为复验其他文件及无关目录后仍返回该冲突；数据库/锁错误依旧立即停止，不改变两个UUID拒绝规则。新增无关目录新文件必被发现且A/B不续接回归。连续性15项、workspace200 Rust（含壳）、严格clippy通过，前端源码未变复用146 UI/API，fmt及release通过。

新conflict-isolation候选EXE SHA256 `31611986780DE45863C453C7F2F1083746D5AA3F8917D425D6BFA2E5550AC9FF`、ZIP `2D1CE7BE3BE78C151D3A488411F468CAEEC18F937A85423C2378535F12A6E74F`，ZIP内EXE一致。直接启动交付EXE PID40620，设置正常显示撤权位置及关系保留，native-smoke.log已存。58的完整同日保存/GUI另存/撤权不冒充本新构建重跑；本构建U8仍待补。

临时源已有UUID长期消解仍未实现，具体可审阅提案见[冲突决定](WINDOWS_CONTINUITY_CONFLICT_DECISION.md)。已有规则禁止静默合并/退休纯扫描UUID，新增生命周期或暂存发现须有明确依据；提案未被当作批准或完成。跨日、原生100k可见性能/SQLite中断及窗口/DPI等关口仍缺。

### 316构建原生离线及同名重建负向验收

目标续行提示不构成对自动处置B提案的批准，继续保持拒绝规则。新增example只读inspect-discovery定位真实WPS临时源未收录原因：暂停时临时源已替换到正式路径，reconcile成功、扫描0失败，详见WINDOWS_WPS_SOURCE_CAPTURE.md，不冒充精确竞争关闭。

原生选择器焦点辅助增加当前前台线程连接后，地址栏确实读回新独立native-offline-4aec…目录；正式选择文件夹授权，明确同意追踪并收藏TXT，Object ID建立。正常关闭316构建后，校验移动前后绝对路径均在已批准测试根且目标不存在，把整个夹具目录改名至-unavailable；直接重启同交付EXE PID41032。原生设置显示“目录暂不可用”但追踪同意与关闭按钮仍可读；点击关闭后只读库active1/paused0/enabled0、原UUID7105e464及favorite1/anchor保留，fsutil标记c68f1945…仍存在。见316-offline-settings/disabled/state。

正常关闭后按相同边界校验恢复原路径，重启316构建PID44896，正式入口重新明确启用。将原TXT移到同一测试根外层独立retained文件，再原路径新建合成另一文档；正式扫描原UUID保留favorite1/status identity_unconfirmed，新UUID61489f96 available/favorite0，binding_history0。收藏UI明确“身份变化，请确认原件”，无直接打开按钮、保留重新指向；尝试查找打开按钮不存在为正确保护，不计打开调用成功。设置显示“文档绑定冲突，请确认原件”，不虚报标记成功。原文件正文WORKTWILL_SYNTHETIC_OFFLINE与新文件INDEPENDENT_SAME_NAME_RECREATION_316独立保留，见316-recreated-list/state/favorites/tracking。没有删除/合并资源或重写旧观察。

本轮未修改产品运行时代码，只改诊断example及工具，316交付构建指纹不变。完成两个负向原生关口不等于完整U8/跨日/性能或竞争自动处置通过。

### 原生十万条搜索、运行中断及验收隔离配置（B4A0ED5E）

新增显式WORKTWILL_VALIDATION_DATA_DIR：必须配合已确认WORKTWILL_TEST_ROOT，目录绝对路径经canonicalize后严格位于测试根下且已存在。正常启动仍用系统app_data_dir；不改既有验收数据库。新增隔离example生成100000条合成文件元数据，正式RebuildSearch投影，授权根暂停实扫；不是10万真实文件扫描。首轮example误写roots表失败，修正scan_roots后新建独立目录成功，原失败日志保留。

SearchCancellation增加VM执行/实际中断计数，壳只在显式隔离配置下输出request_id、耗时和计数，不记录查询、名称或路径。正式交付EXE B4A0ED5E3657F9486ABD29C0841E5A49464F3A4CC41E1DE32D2422F87C8D10D4，ZIP815E0EE167DCCE2AFA8F7077FA07D84DA09FAF92D22E46148681F859F6293424。直接启动该EXE PID40372，数据目录native-search-100k-6c119300-b4b4-42d8-afc6-3b72dd756669。

正式原生UIA输入查询“部”后真实键盘Escape，捕获24条vm_interrupts=1且vm_progress_ticks>0，未注入慢SQL、未用mock IPC；随后查询“张三”结果正常。初轮docx查询过快，12次尝试未捕获中断，不记为通过。原始计数native-search-execution.jsonl、命中native-running-interrupt.json、恢复native-post-interrupt.json。

每轮50次10组查询，计时从UIA ValuePattern输入开始，至首个search-result-0存在且非IsOffscreen；包括40ms正式debounce和IPC/SQL/React/UIA观测，但不是像素呈现时间。中文P50/P95/P99为125.78/149.07/165.65ms。英文与完整检查并行时141.06/179.80/216.02ms，P95失败保留；检查结束独立复测114.16/134.92/142.32ms，全部找到结果。仅当前夹具/环境/查询组合，不能泛化所有负载或宣称已测像素SLA。

聚焦保留三轮失败/有歧义记录：SendKeys同步耗时P95 82.95ms；直接按键未固定起始焦点4次超时；固定非搜索焦点仍1次超时。最终每次显式激活前台并确认管理设置按钮持有焦点，再直接keybd_event Ctrl+K，30/30成功，P50/P95/P99 8.23/15.96/17.19ms。中途commentary误报受控轮全部成功，逐条复核后已纠正29/30。按原记录判断，不删除异常。

完整pnpm check退出0：146 UI/API、201 Rust（含壳23项，5规模默认忽略），DTO/build/fmt/严格clippy通过。取消4项回归另轮通过，不与全套重复累计。native-validation-full-check/build及candidate.json存档。本构建完整U8未跑，58旧保存证据不挪用。真实跨日、自然临时源竞争与长期处置、其余窗口/DPI和最终审计仍未完成；目标保持active。

B4最小窗口补证：UIA Transform.Resize(900,640)实际受125%DPI换算为1125×800，未误记900×640；随后MoveWindow并读回900×640物理尺寸、DPI120，保存中英设置及英文搜索截图/UIA。长目录路径换行、管理操作/常驻搜索可访问，设置纵向滚动；不代表全部DPI、超长文件名、对话框矩阵。修正后文件名含physical900，原大尺寸截图保留。独立100k测量进程已正常关闭，后续B4用默认应用库继续保存验收。

### B4构建同日保存续行

默认库重启B4，正式文件夹选择器重新授权原合成native-continuity目录，先确认原UUID79c9737a、双区2和收藏1。实际WPS GUI Ctrl+S首段B4FINALRUNNINGONE落盘；第二段直接SendKeys被拼音输入状态影响而未完整落盘，最初offline尝试也未完成，两个body负例保留。改为剪贴板粘贴标记并GUI Ctrl+S，运行中B4_FINAL_RUNNING_TWO_VERIFIED、正常关闭WorkTwill后B4_FINAL_OFFLINE_THREE_VERIFIED均由共享只读ZIP/XML核对存在，再启动同B4 EXE PID48120。

重启后原UUID不变、favorite1、links2、同名资源1、绑定历史13。首次关闭文档遇WPS“是否保存”对话框，u8-A.png实际是该对话框，不登记为重开正文；原生UIA点击保存后标题变成另存为独立验证58.docx，确认目标关闭。随后分别从工作区A、B、收藏正式打开原件，每次关闭前后观察其他标签标题，三张*-reopened.png显示包含三段B4正文。UIA异步页面切换过早曾未找到按钮，等待树更新重试成功，没有改数据或人工换绑。A/B/收藏分别打开取得同日证据；不等于真实跨日或源竞争长期处置完成。后续关闭确认额外保存使实际版本数增加，以final-state为准；原始活动身份不改写。

B4长文件名验收新发现与修复：创建独立合成TXT（字符数/路径见b4-long-file.json），正式扫描与中文子串搜索、正式打开到记事本，UIA读回正文正确。900×640物理窗口/125%DPI英文搜索截图暴露面板左缘裁切Search scope和快捷键开头，按失败记录。根因为窄窗搜索面板按视口最大宽度且右对齐，超过输入框及内容左缘；CSS仅把≤760 CSS px下的面板设为输入框100%宽，新增简短中文原因注释。修正后构建及原生复验继续；这会产生新指纹，B4保存/性能证据保留原构建归属。

纠正上一段记事本边界：进程标题证明正式打开到记事本，但读正文尝试因Edit控件未找到失败，未生成body.json，不能登记UIA正文正确。原文件正文创建值仍可只读核对，需重新打开取正文证据。此失败不影响搜索裁切已由截图确认。

### 按用户最新要求收敛验收范围

用户明确“测试验收要注意提高效率，不要过度极端场景，有bug我们后期可以再解决”。后续以正常日常流程及数据/授权安全为重点；停止扩展低概率强制WPS时序、更多压力负载和DPI排列。已有UUID竞争仍保守拒绝，作为已知限制后续处理，不需要新增自动合并/退休规则，也不据此宣称自动恢复成功。并行负载超预算、未验证的像素呈现时间、扩展DPI不再继续追测，保留原结果。真实跨日与Word仍未验证，不因范围收敛改写为通过。

窄窗修复96D39A256F3F1EBA09870A266DECAAF672750DC5613042FD949778A5FC6B95E1实际启动PID49048，900×640物理/125%DPI英文长文件名搜索截图确认面板左缘、Search scope及全部键盘提示完整，裁切修复通过。本变更仅CSS，release build通过；复用此前B4完整146 UI/API、201 Rust结果且明确构建区别，不再无关重复全套。最新EXE/ZIP及指纹见narrow-search-candidate.json。B4同日保存三入口原生证据仍归B4；96未重跑完整保存，不混称最终构建全矩阵通过。整体仍为可试用工程候选，未将全部V1验收标为完成。

### 96最终候选日常流程复验与交付

用户要求继续，按收敛范围完成，不扩大极端场景。此前被中断的ZIP校验已成功写出zip-check.json：内嵌EXE SHA与96D39A25一致。最新96交付EXE运行中，先有双工作区+收藏，在真实WPS GUI粘贴FINAL96_RUNNING_SAVE后Ctrl+S，XML读回确认。正常关闭WorkTwill PID49048，在WPS写FINAL96_OFFLINE_SAVE并Ctrl+S，XML两标记均存在。直接启动相同EXE PID45452，正常关闭目标WPS标签，标题切至另存为独立验证58.docx；随后分别从工作区A、B、收藏正式打开原件，每次之间关闭目标标签并确认其他标题，三张96-*-reopened.png显示两标记正文。重启只读库原UUID79c9737a不变、favorite1、links2、同名资源1、available。未用COM保存或手工换绑替代。

最新制品/指纹保持narrow-search-candidate.json，更新为该精确构建同日日常保存取回已验证。范围：Win10 LTSC19044 x64、内置C盘NTFS普通目录、WPS12.1.0.29690、明确启用追踪。真实跨日、Word仍未验证。低概率已有UUID竞争、扩展压力/DPI按用户最新要求后续处理，现有竞争保持保守拒绝，不登记自动成功。签名安装与发行准备原先已后置。交付可试用工程候选，不声称未实测的完整矩阵或全部V1完成。

### Microsoft Word实际验证尝试（用户追加要求）

本机现已发现真正Microsoft WINWORD.EXE，路径C:/Program Files (x86)/Microsoft Office/root/Office16/WINWORD.EXE，Microsoft Corporation、16.0.19127.20936，Authenticode Valid；不沿用此前Word.Application指向WPS的推断。96构建下新建独立native-word-b587…夹具，正式目录授权、另行同意追踪、收藏并加入A/B工作区，UUID b8fba2e9。Word真实窗口能加载DOCX（兼容模式），但首次启动OfficePLUS安装、获取正版Office及隐私提示阻挡编辑；拒绝可选加载项安装，普通/安全模式启动均未取得已落盘测试标记。保存尝试XML false及提示截图保留，不能登记Word保存、重启取回通过。没有修改许可证、安装组件、换默认打开程序，也未用WPS代替。Word当前为环境启动/许可提示待处理，验证未完成；需先让本机Word正常进入可编辑并保存状态后继续同夹具，不重复构建WorkTwill。
