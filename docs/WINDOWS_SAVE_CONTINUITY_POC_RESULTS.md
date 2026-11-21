# Windows 保存连续性 PoC 第一轮结果

2026-11-21。用户确认按审查方案尝试，本次将创建 Object ID 的授权限定在新建合成样本。正式身份代码、默认元数据策略和生产数据库均未修改。

## 实验条件与边界

独占目录：`C:\Users\Administrator\AppData\Local\Temp\worktwill-windows-v1-20261121\save-poc-c1b9d795be4f4f35bd96d5f17862216a`。Windows/NTFS环境沿用主验收记录。通过本机已注册到 WPS 的 `Word.Application` COM 创建并保存 DOCX，COM Version 返回12.0；实际编辑器版本沿用先前已核验的WPS 12.1.0.29690，不能称Microsoft Word实验。本轮执行COM Save，而非人工Ctrl+S。

观察器为.NET FileSystemWatcher，保留Created/Changed/Deleted/Renamed原始回调、顺序、接收UTC与单调时间、Error；这是Windows通知的观测，不是notify完全相同的事件格式，也不是内核原始事务日志。观察器缓冲64KiB。身份在阶段边界通过只读fsutil查询；没有采集到每个瞬时临时对象身份，不能回填成完整对象因果图。日志写在同目录，包含snapshots.jsonl自身变动，分析时须明确排除这些观测输出；本次没有报告Error不代表证明绝无丢事件。

## 实测结果

| 项目 | 结果 | 可得结论 |
|---|---|---|
| 无Object ID控制组连续3次WPS Save | 每次File ID变化，均没有Object ID | 重现既有File ID连续性缺口 |
| 创建Object ID组连续3次WPS Save | 每次File ID变化，Object ID始终为f47a19457be5f111921cf4a475006fad | 标记在该保存组合中可保留，值得作为补充证据 |
| 普通复制 | 副本没有Object ID，原件保留 | 该复制样本未继承标记 |
| WPS SaveAs2副本 | 副本没有Object ID，原件保留 | 该另存样本未继承标记 |
| 同卷文件系统改名/移动 | Object ID保留 | 本次由PowerShell Move-Item执行，不登记Explorer真实手势通过 |
| 停止观察器后WPS保存 | File ID变化，Object ID保留，正文新标记落盘 | 证明标记可在无本观察器时保留；未证明WorkTwill离线恢复或跨日U8 |
| 模拟临时文件替换链 | 记录到旧目标改名、临时文件改为目标、旧备份删除 | 路径事件可以被替换操作制造，未证明与WPS全部特征完全相同 |
| 另一份DOCX通过File.Replace覆盖带Object ID目标 | File ID变化，但目标原Object ID仍保留；正文改为另一控制文档的标记 | **Object ID相同不足以排除无关内容替换，不能单独批准自动继承** |

最后一项是直接反例：目标Object ID `147b19457be5f111921cf4a475006fad` 在覆盖前后相同，File ID从 `...060000000226f1` 变为 `...08000000022767`，正文成为 `WORKTWILL_POC_control_SAVE_3`。这是正常ReplaceFile语义，不代表Windows标识损坏。若产品接受“原路径文档被ReplaceFile更新即连续”的语义，则必须明确其会接受这种覆盖；当前PoC没有批准改变原语义。

## 失败操作与未覆盖项

首次跟进脚本在关闭文档后COM对象不可用，停止观察器后的保存未执行；重新创建COM连接才完成。首次.NET File.Replace传null备份路径在PowerShell绑定为空字符串，报path empty；其后的after快照实际上未替换，不能计成功；改为明确备份路径，`replacefile-unrelated-after-success`才是真正替换后结果。所有快照保留。

尚未实施影子分类器、ContinuityProof、独立Store绑定更新、收藏/双区/最近使用/历史权限核验、真实GUI连续保存、应用退出/重启自动恢复、跨日、崩溃/溢出/撤权、标准用户权限及USN探针。正式V1五项清单不能因此登记完成。

## 后续判定

第一阶段得到两个有效结论：Object ID可以补充本机WPS的保存证据；仅Object ID或仅rename链均不足以满足当前“不得误继承无关替换”的门槛。先保持生产策略不变，不以本轮正例直接自动换绑。

下一步应围绕这一反例检验可用证据是否真的增加区分能力；若没有，不能用多项弱特征打分冒充确定性证明。需要明确保存覆盖的产品语义，或找到经过验证的更强文档延续依据，然后才进入实验Store关系验证。

原始证据位于同日期 `docs/validation/2026-11-21-windows-save-poc-*`：events/identities两组JSONL、合成DOCX正文标记核验JSON、观察器源码及首轮执行脚本。执行脚本是本轮运行快照，依赖本地fixture指针及已创建样本，不是可直接重跑并覆盖原夹具的测试入口。
