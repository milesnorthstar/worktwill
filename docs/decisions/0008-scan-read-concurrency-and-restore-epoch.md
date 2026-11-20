# 0008：扫描写入与快照读取并发，恢复作废旧枚举

日期：2026-11-20。状态：迭代009实施；性能结论以验收记录的实际测量为准。

100k实际合成文件的初扫并发搜索暴露全局锁竞争。旧 Catalog 每100条记录持有 Store.gate 独占锁直至 metadata、FTS、活动和 revision 提交，search 的共享锁需要等待写入。此前不变重扫的低延迟不能代表首次建立 FTS 时的体验。逐阶段 oracle 补齐后的基线见验收记录。

批次现在按 `catalog_write Mutex → gate.read → BEGIN IMMEDIATE` 排队。mutex 仅串行同一 Store 的 Catalog 批次；不能先拿 gate 再等 mutex。shared gate 仍阻止恢复、撤权、暂停、排除和其他独占操作穿过当前批次；它允许持有读事务的查询与 WAL 写入并行。IMMEDIATE 在读取 valid、物理身份与旧 metadata 前取得 SQLite 写保留锁，避免先读再升级的快照冲突。UUID、关系、双 FTS、事件及 revision 仍在同一事务内提交；当前 metadata/物理身份/路径复验保留。

setup 与 finalize 继续独占 gate，控制写入与恢复也继续独占；没有反向取得 catalog mutex 的路径。独立进程或另一 Store 实例不受这个内存 mutex 保护，由 SQLite 写锁及既有 timeout 处理，不宣称跨进程授权序列化。

search、完整搜索与 timeline 已有读取事务。overview、resources 和 file_grants 也改为单个读取事务，避免候选顺序、逐项字段、概览组成与版本来自不同批次。DTO/业务状态归属不变，不增加前端缓存事实源。新增测试在正式准备版本 SQL 的 authorizer 回调中协调另一 SQLite 连接实际提交，核对响应仍完整属于旧快照，下一次读取才显示新版本；回调仅测试构建可用，不替换查询结果。

审查同时复现恢复 generation 的 ABA 问题：旧扫描为10，恢复generation=8的备份后变9，重新授权又为10，旧批次会错误通过valid。新增进程内 scan_epoch；扫描开始时与数据库generation一起捕获，检查、批次提交和finalize都核对；成功恢复在独占gate释放前递增epoch。它不从备份恢复，进程退出后旧扫描线程也不存活，无需新增持久schema字段。恢复失败不登记成功epoch。已有权限/revision严格递增策略保留。

新增回归在旧实现分别复现“读租约阻止批次提交”和“恢复重授权后旧代次复活”，失败日志保留；修复后通过。规模验收现在逐阶段检查资源数、不同资源的发现事件数、修改事件归属、根状态；比较全部资源的 UUID/路径/名称/身份/mtime/size/状态，预期只有目标文件metadata变化。只对目标原件验证内容，不宣称逐个核对所有文件字节。

初扫中非空查询前后都核对根仍 running 且未扫描完；查询时延只计 search 调用，不含该辅助检查。报告明确总查询数、最多前10k条保留样本和在途尾查询，不能把截断分位数称为任意长测的全程分布。这仍是合成小文件的后端扫描与搜索测量，不是原生渲染、Office、冷磁盘或空闲资源验收。
