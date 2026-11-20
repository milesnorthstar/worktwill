# 0016：按读取版本复用完整月历计数

日期：2026-11-20。状态：迭代018实现、统一检查及完整规模正确性复测通过；首次月份和深offset性能仍单列未解决。

017分段确认每次翻页重新计算整月日历约290ms。增加每Store独占、最多4项的LRU缓存，只存最多31点的整月计数，不存资源正文、事件列表或授权集合。每次timeline先在同一读事务读取permission_version/revision建立快照，再使用版本、完整day_boundaries、filter、basis、workspace_id匹配。Store/session隐含于缓存所有者；不同Store不共享缓存。month起止由已校验日界覆盖；列表from/to/offset不影响整月日历，不进入key。

短锁仅用于查找/插入/淘汰；未命中SQL在同一读事务计算且不持缓存锁。旧请求晚插入旧版本条目仍受精确key隔离，容量仍为4。恢复后的严格递增版本与新Store空缓存隔离旧结果。不为缓存增加独立持久数据、定时刷新或静默兜底。

只读审计未发现改变日历计数但不更新版本的正式写路径：mutate、Catalog提交、grant_root、ingest、record_open、relink/merge均同事务bump，恢复两个版本各增一。扫描开始的generation/状态统计、refresh_file_grants失败时的资源status、打开Store改partial_failure不影响当前VISIBLE和计数；这不是缓存正文的依据。

直接SQL的测试helper必须遵守版本契约：Fixture::insert改为事务内插入并bump；原单文件100k→500k规模基准每档seed事务bump，防止第二档误复用第一档缓存。它的报告另外保留首次未命中及热样本使用缓存标志。该单文件基准本轮尚未执行，不冒充新证据。

定向11项时间线回归通过：新增全条件key隔离、from/to/offset复用、4项LRU淘汰、无版本status变更仍重读正文，以及Transfer/Exclude/Include/活动写入/恢复撤权的缓存失效。原撤权、工作区、双时间、夏令时/半开边界用例保留。完整100k混合库996搜索页+976时间线页、2,239独立oracle复测零错；报告显式记录第一次正式API日历未命中292.500ms。时间线热首页/100/10k/末页P95为2.617/2.674/32.995/301.891ms；全链P50/P95/P99为153.523/287.518/436.390ms，1099次日历命中。不能用热命中掩盖首次打开或深offset。反向关系索引也改善名称全链P95为2.368ms，名称四深度均低于60ms。
