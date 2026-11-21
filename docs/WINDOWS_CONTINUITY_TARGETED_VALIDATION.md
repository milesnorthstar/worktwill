# 连续性定向验证：扫描时序与混合授权

2026-11-21。只修改隔离example和必要回归测试，尚未实现正式自动续接服务。

## 混合授权

新增 `same_path_replacement_retires_old_single_grant_even_with_later_directory_grant`：先为文件单独授权，再授权其目录；将旧对象移出目录，新建同路径替代物；通过现有人工relink事务换绑。验证旧file grant消失，目录仍有效时可访问，撤目录后返回not_authorized；新旧正文均正确保留。定向测试通过，整个relink_grants套件9项通过，example与测试严格clippy通过。

这证明人工事务已有的撤旧grant边界正确；不能当作自动续接通过。与前轮仅SQL换identity后仍泄漏授权的负例形成对照。自动服务须复用此授权语义，但不能直接后台调用人工relink冒充自动证明。

## 强制替换源先入库

用独立Store先扫描真实DOCX副本`contract-save.tmp`，明确取得资源B，然后通过File.Replace替换原A。这是受控文件系统操作，**不是实际WPS在.tmp阶段被暂停的重现**。此前WPS12次保存+300扫描未捕获该精确阶段，仍保留未验证。

两组对照揭示当前物理identity含创建时间带来的差异：

| 组 | 替换前条件 | 实验结果 |
|---|---|---|
| 普通副本 | B创建时间与A不同 | ReplaceFile结果采用B的File ID但保留目标创建时间；完整identity不等于B原identity，因此候选策略接受更新，资源总数仍2，留下另一条扫描记录 |
| 相同创建时间控制组 | 在合成样本上显式设B创建时间等于A，再扫描 | 替换后完整identity等于B已登记identity，候选策略reject_existing_resource，A关系保留待处理 |

对应证据：forced-source-before/after.json与forced-source-equal-birth-before/after.json。第一组不能声称没有冲突问题：它仍有陈旧扫描记录；第二组证明现有冲突保护可能阻断此类顺序。不能因为B没有收藏就静默合并，也不能删除创建时间维度来绕过检查。两组并不证明WPS必然采用相同创建时间行为。

## 实现约束

连续性解析必须区分物理File ID和防复用的创建时间组合，登记候选源的观察来源和绑定历史，不能靠一次完整identity查询覆盖所有冲突。是否处理纯扫描记录仍需真实WPS目标时序证据与明确规则；本轮未新增生产例外。

正式自动服务、历史lineage、跟踪同意UI、迁移恢复与原生U8还未完成。后续验证必须在这些实现之上执行，不能继续把SQL候选实验登记为正式保存恢复成功。
