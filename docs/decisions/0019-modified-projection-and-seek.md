# 0019：搜索派生修改时间与独立续页位置

日期：2026-11-20。状态：迭代021；完整检查及首轮性能已通过，最终规模迁移与包体证据见ACCEPTANCE。

020默认相关性已优化，但modified排序仍由FTS候选连接资源后全量排序，100k合成库all/file/单日/英文路径/中文路径首页P95为88.773/89.731/86.070/109.238/132.353ms，超过60ms预算。不能把它归为只差原生验收。本轮沿现有搜索投影职责添加派生时间，保持resources.modified_at为唯一业务事实源。

schema v4在同一IMMEDIATE事务执行v3→v4：增加search_documents.modified_at可空REAL列，从资源表一次回填，建立(modified_at DESC,name,entity_id)及(kind,modified_at DESC,name,entity_id)索引，最后推进user_version。历史schema.sql继续是v1，新库统一走1→2→3→4，避免重复ALTER。旧程序的未来版本拒绝机制保持；迁移不把资源0与工作区NULL混同，也不改资源事实、关系或FTS。

search::project显式接收Option<f64>，所有正式消费者由编译器要求传入时间。Catalog使用提交复验的item.modified；导入/重新指向使用写入资源的同一mtime；SaveWeb先取得一次saved_at用于两处；工作区创建/改名传None；rebuild从资源读取时间、工作区为NULL。文本/kind/FTS存在且仅时间变化时只UPDATE搜索投影一行；全部不变时零写入。FTS缺行仍沿原修复路径，错误内容仍由显式重建修复。未新增触发器或另一个状态所有者。

高命中modified查询保留最多2049条FTS探测，再用时间索引流式取可见项；类型查询用kind时间索引，避免稀少类型穿过全部文件。排序明确为d.modified_at DESC,d.name,d.entity_id，全部授权、范围、查询、日期和字面路径条件仍在LIMIT前。首页先非NULL，再NULL工作区；numeric续页依次为同时间且(name,id)>位置、较早时间、NULL尾段；NULL位置只在NULL段按name/id继续。每段只取补足101条的剩余数量。零与负值都是有效numeric时间。日期条件直接形成索引范围，较早阶段取min(cursor.time,criteria.to)作单一更紧上界。三个阶段不合并成大OR；测试解释器共享正式runner并标记阶段。

低命中继续FTS，使用相同的复合位置条件；候选有界，允许小集合排序。完整搜索全部排序不再用OFFSET跳过前页。SearchCursor协议4新增modified_position{modified_at nullable,name,id}，与name/rank位置严格互斥；外层缺失是无效游标，内部NULL是合法工作区尾段。后端在同一读取事务从实际第100项的投影读取位置，不能从UI工作区modified_at=0反推。原条件/session/权限/revision/next_offset验证及取消保持；mtime更新照常使旧revision游标失效。

独立审查逐项核对SQL绑定、NULL/时间排序、授权筛选、迁移及mtime消费者，未发现确定缺陷；审查本身不计作测试。新增真实历史v3结构回填、ALTER后故障回滚重试、重复重开不回填、真实v3备份恢复重建/撤权/源备份不改四项。新增投影mtime-only一次写入及FTS修复，生产写路径贯通到重建/恢复，380条低命中含200个NULL工作区完整链，以及高命中四相关性档/零负时间/多页NULL/8筛选组合。阶段计划要求Equal与Null续页按名称/ID定位、Earlier直接按时间范围，无临时排序；旧协议与三槽混用、非有限及超界时间拒绝。

最终检查数量、100k完整分页、9案例首页、10k实际扫描和迁移初始化耗时见ACCEPTANCE及原始JSON。任何热查询数字均不是原生IPC或可见延迟证明；月份首次计数、文件locator、Office安全替换及发行仍须独立闭合。
