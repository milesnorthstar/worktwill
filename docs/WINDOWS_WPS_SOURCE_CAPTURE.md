# 真实WPS临时源受控观察

2026-11-21。仅使用已批准测试根中新建的两组独立夹具；正式Store与当前debug核心，不是交付EXE正常保存性能验收。没有改动生产冲突消解规则。

1. `wps-source-capture-598c80cc4512431f8ccb6b37d3b71b06`：WPS真实GUI保存产生`contract~2114E.tmp`。FileSystemWatcher回调打开临时源，不共享删除，扫描后释放。正式扫描确实建立临时源UUID `6e9b9711-27a7-4323-9977-dd778c9f2ee1`。保存结束后目标采用该File ID但创建时间为旧目标的创建时间，**Object ID丢失**，原UUID保留双关系与收藏、状态identity_unconfirmed；另一目标UUID被扫描发现。此干预改变了WPS替换行为，不证明无干预保存也会丢标记；不得记为连续性通过或正常保存重现。见real-source-capture.log、real-source-after.json、real-source-state.json。
2. `wps-suspended-source-cec946debef5488c8bf023789e3c10fc`：回调NtSuspendProcess后立即扫描，finally恢复。创建与Changed两次捕获均suspend=0/resume=0；正式扫描未收录临时源，目标Object ID保留，后续正常连续性恢复。**没有捕获临时源可读并成功入库的阶段**，不能登记精确竞争通过。见suspended-source.log、suspended-changed-retry.log、suspended-state.json。前一次Changed尝试没有事件日志，保留为未取得结果。

探针源码已归档source-share-probe.cs、source-suspend-probe.cs。阻止删除和暂停进程是显式实验干预，两者不能混同自然时序；已存在的两个UUID仍拒绝自动合并/删除/退休。后续应捕获可识别的真实替换源，或设计不产生正式第二UUID的发现阶段；后者若改变正式收录行为须明确依据和覆盖，不用文件名猜测原文档身份。

19:10新增inspect-discovery只读诊断，逐路径报告metadata/canonical/identity/reconcile及扫描状态。读权限可用后暂停的真实GUI保存事件contract~9C467.tmp中，暂停完成时目录已只剩contract.docx与锁文件，目标reconcile=Ok(true)，扫描completed/failures0。说明该轮未收录tmp是因为替换已完成，不是扫描无故忽略一个仍可见且可识别的临时源。见wps-discovery-diagnostic-retry.log；首轮键盘未产生事件没有结果，未计成功。此诊断仍未关闭真正源先入库竞争，未批准自动处置已有B。
