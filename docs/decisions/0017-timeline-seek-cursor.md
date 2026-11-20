# 0017：时间线使用读取版本绑定的末项游标

日期：2026-11-20。状态：迭代019统一检查及最终规模复测通过；原生界面与首次月份成本继续单列。

018月历复用后，时间线末页P95仍301.891ms，事件SQL每页OFFSET跳过全部前页。新增TimelineCursor协议1：Store session、permission_version、revision、next_offset、完整TimelineCriteria以及实际第100条事件的before(effective_at,id)。criteria覆盖from/to、month_from/to、全部day_boundaries、filter、basis、workspace。next_offset只是逻辑计数；不持有跨请求事务或服务端游标登记表。

第一页offset0不带cursor；后续页必须携带同条件/同读取版本的cursor。缺失、过期、条件或版本不匹配、无效位置明确stale_cursor，不静默回退OFFSET。位置不是签名授权，合法构造的位置可改变翻页位置，SQL仍每页核对完整授权/工作区/类型/日期。正式入口先读版本建立同一SQLite快照，再校验游标、读取事件/资源、复用或计算月历和签发下一页。恢复严格增加版本，重开Store更换session；都不能沿用旧cursor。

SQL按(effective_timestamp,a.id)<before继续，保持双键DESC和100+1 lookahead。文件修改时间仍用既有CASE，首次发现/打开按观察时间。observed补派生索引(at DESC,id DESC)，新旧v3及恢复staging通过同一migrate补建，不改业务版本。原有CASE表达式索引保留。

首轮完整2239 oracle通过，末页由301.891降到53.275ms；实际计划表明SQLite没有把tuple内的CASE表达式时间有效用于范围定位。补充等价标量effective_timestamp<=before.at，让表达式索引直接定位；tuple继续处理同时间戳ID边界。仅加标量上界实测仍约54ms，未解决两个日期/游标上界竞争。续页保留原日期上界为残余校验，让索引单一采用游标时间上界；最终末页热P95为1.306ms，完整976页P50/P95/P99为2.487/2.774/4.223ms。该选择不取消原日期约束；伪造合法时间位置也不能绕过授权或选日范围，另有专用回归。首轮与中间失败性能报告保留。

React原样回传next_cursor，合并后保存新响应的下一页位置。仅append的stale_cursor触发一次首页重读；首页失败显示原错误，不循环重试。原有sequence检查保持，过期append不能污染新筛选/新游标。没有通过前端自行计算位置来绕过后端版本。

核心回归覆盖16种缺失/条件/版本/位置拒绝、实际mutation/reopen/持游标恢复后失效；两时间维度225项混合事件、同时间戳三页完整链与实际索引范围计划。UI新增续页stale一次恢复、首页stale不重试，以及旧append晚到后新筛选继续使用新cursor。最终全套数量及日志以验收记录为准。

100k验收器和诊断均从真实链保留每个offset对应的request/cursor，深页不得只改offset。profile_timeline仍为cfg测试诊断：不执行正式cursor校验、强制重算日历；它只用于共享SQL分段和计划，不能替代正式带缓存/校验入口的总调用SLA。原生IPC、文件变化到窗口与真实可见延迟继续未验收。

最终100k混合库996搜索页99,599项、976时间线页97,533项、30日历点，2,239独立核对零错。时间线首页/100/10k/末页30热样本P95为2.472/2.699/2.604/1.306ms；首次未命中月历288.760ms单次，不称冷盘或P95。完整检查104项Rust/100项UI通过，纽约时区100项实际重跑通过。具体日志/包体指纹见ACCEPTANCE；仍不是原生IPC或可见时延证明。
