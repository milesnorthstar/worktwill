# macOS Document ID：只读能力、生成条件与身份连续性

日期：2026-11-20。独立研究，不修改生产代码、既有文档或旧样本。

## 结论与可实施范围

本机 APFS **支持 Document ID，但普通新文件不自动拥有非零 ID**。本次两个独立进程、三个新样本的六次查询，Foundation 均成功但返回 nil；底层 `getattrlist` 均成功、返回属性位存在而 ID 为 0。先前 nil 的现象与“尚未分配 ID”一致，不能直接解释为 APFS 不支持、Bookmark 创建失败或 API 参数遗漏。

在原件 metadata 不写入、正文不读取的约束下，未找到能够为任意现有文件普遍建立、且可靠跨进程关闭与 inode 替换的原生文档身份机制。**没有足够证据将 U8 身份变化的安全保存列为通过。** 这不是把零值作为卷不支持，也不是否定已存在 Document ID 的辅助价值。

建议当前可实施的边界：

1. 保持物理身份一致时的自动恢复与现有 Bookmark/根定位器行为。原始物理身份变化时，现有防误连不降级。
2. 若后续实现能力诊断，独立区分：卷能力未知/不支持、属性获取失败、属性未返回、返回 0、非零 ID；只持有**卷 UUID + 非零 ID**，不能只持有裸整数。诊断不触发 `UF_TRACKED` 设置，也不以路径、mtime、Bookmark 成功补足缺失证据。
3. 非零 ID 只能先作为候选证据。Apple 的 HFS 官方测试明确允许“删除原文件后同名创建新文件”继承 ID，因此“非零且相等”不天然排除误连。先完成本机 APFS 的已有 ID 正负例，再决定它能否在产品所定义的连续性范围中作为权威证据。
4. 如要普遍支持原地替换，必须重新评估约束或交互：由编辑器/系统预先提供可靠身份、采用用户明确同意的追踪 metadata 方案，或对未知替换执行人工确认。后两者会改变原件只读承诺或 U8 无反复修复目标，不能悄悄当作已满足现要求。即使允许 tracking，也仍需删除重建负例，不能把赋 ID 当作完整解法。

## 官方公开语义

[Apple URLResourceKey.documentIdentifierKey](https://developer.apple.com/documentation/foundation/urlresourcekey/documentidentifierkey) 的 Discussion 明确：

- kernel 分配给文件或目录；同卷移动后仍可识别。
- 跨系统重启；复制文件不传递 ID。
- 跨 safe-save 存活，保持在原先被赋 ID 的路径上；推荐的例子是 `FileManager.replaceItem`。
- 只在同一卷内唯一；并非所有卷支持。

本机 SDK `Foundation/NSURL.h:256` 的公开注释一致。该字段是 read-only NSNumber；**read-only 表示不能通过该 key 设置，不等于任何文件系统上的 getter 绝无内部 metadata 副作用**。

`NSURLGenerationIdentifierKey` 用来判断同一 inode 数据是否变动，不解决跨 inode 身份；`NSURLFileIdentifierKey` 文档告诫不应持久化；APFS `NSURLFileContentIdentifierKey` 可被 clone 与原件共享，不是唯一逻辑文档 ID。

## 生成条件：明确事实与限制

本机 SDK `sys/attr.h`：

- `VOL_CAP_FMT_DOCUMENT_ID` 定义为跨 object-ID 变化的文档修订身份能力。必须同时检查 capabilities 与 valid 位；不能以文件系统名称代替能力检查。
- `ATTR_CMN_DOCUMENT_ID` 必须传 `FSOPT_ATTR_CMN_EXTENDED`；未传时与历史属性位冲突。
- `sys/stat.h` 定义 `UF_TRACKED=0x40`，说明其用于 Document IDs；标记后的删除/改名通知行为也发生变化，因此不应作为无害的隐式补丁。

[Apple XNU vfs_attrlist.c](https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/bsd/vfs/vfs_attrlist.c)：

- 571 行将此属性映射到 `va_document_id`，授权动作是读取属性。
- 2006–2012 行按文件系统是否返回支持位打包 ID；可返回零值。消费者需要检查返回属性位，不能把未返回的缓冲区零与已返回的零混为一谈。
- 3341–3349 行在没有 EXTENDED 选项时返回 EINVAL。

[Apple HFS hfs_vnops.c](https://github.com/apple-oss-distributions/hfs/blob/d1bac2f062e6e9c0dfcce302d9aacb10173d0eea/core/hfs_vnops.c)：

- 747–752 行：可写卷、已设 `UF_TRACKED`、非卷根且合适文件类型才满足“应生成文档 ID”。
- 833–886 行：读属性时，若尚无 ID 且满足上述条件，会分配 ID、设置 `C_MODIFIED` 并发出变化事件。因此不能笼统保证 getter 在所有受支持卷上无内部 metadata 写入。
- 1277–1295 行：首次设置 tracking 时，无 ID 则分配/设置 ID。这是 metadata 变更，不符合本任务不设置原件 flags 的约束。本次没有执行该操作。

**这些是 HFS 实现事实，APFS 实现并未公开，不能直接将 HFS 生成条件写成 APFS 保证。** 本次 APFS 实测只证明普通未跟踪新样本的状态；未实验设置 tracking，也未检查真实 WPS 旧文件。

## 不能遗漏的删除重建负例

[Apple HFS test-doc-tombstone.c](https://github.com/apple-oss-distributions/hfs/blob/d1bac2f062e6e9c0dfcce302d9aacb10173d0eea/tests/cases/test-doc-tombstone.c) 是官方测试源码，本次只读取，没有运行：

- 179–208 行：“Delete original file, move new file into place”，预期继承旧 ID。
- 241–263 行：“Delete original file, create new file in place”，`unlink` 后 `open(O_CREAT)`，预期 ID 与原文件相等。
- 对应 HFS 实现中有路径/线程相关 tombstone 的继承逻辑。

所以官方 safe-save 的“sticky path”语义本身就比“用户语义上的同一文档”宽。它可以提供 OS 定义的连续性，却不能凭公开承诺排除所有同路径新文件。当前 PRD 的未知替换防误连要求更严格；需要实际受支持卷/编辑器的测试和明确定义，不能用文档中“survives safe save”几个字跳过负例。

## 本机只读实测

证据：[2026-11-20-macos-document-id-readonly.json](../validation/2026-11-20-macos-document-id-readonly.json)。包含完整探针源、SHA-256、六组原始返回、前后 stat 字段及环境。

- macOS 15.3.1，arm64；卷为 APFS，Document ID 能力有效且支持。
- 已批准根下独占目录：`/private/tmp/worktwill-spikes-20261119/docid-readonly-20261120-ll3b3sfd`。
- 三个全新样本：空 `sample.txt`、空 `样本.docx`、目录 `sample-directory`。`.docx` 只是扩展名，不是 Office 文档或 Office 保存测试。
- 两次全新进程，每次 Foundation + 原始 `getattrlist` 读取。六次 Foundation 均成功、值 nil；六次原始属性已返回且 ID=0；flags 不含 `UF_TRACKED`。
- 六次未带 EXTENDED 的负例均返回 errno=22（EINVAL），确认扩展属性选项确实必要。
- 六次 getter 前后 dev/ino/birthtime/ctime/mtime/flags/size 相同；这仅证明列出的可见字段未变化，不证明文件系统内部没有任何隐式 metadata 更新。
- 没有 chflags、xattr 写、setattrlist、正文读取、旧样本修改、真实 Word/WPS 保存、系统重启、已有非零 ID 或安全保存实测。

## 其他原生能力与停机期间

| 能力 | 可实现用途 | 不能补足的证据 |
| --- | --- | --- |
| Bookmark / file reference | 定位移动后的候选对象 | 成功解析本身不能证明 inode 替换仍是原文档；已存在同路径重建负例 |
| FSEvents 持久历史 | 补扫应用关闭期间变动、降低全量扫描频率 | 事件为 advisory，可能合并/丢失；不是完整 rename 事务或文档语义日志 |
| FSEvents `CreateWithDocID` | 在 file events 扩展字典中取 `fileID`/`docID` | 不承诺给未跟踪文件赋 ID，也不改变 Document ID 的 sticky-path 语义 |
| 同一物理身份 + 根/文件定位 | 现有可安全实现的自动取回 | 不能跨真实物理替换自动认定逻辑连续 |
| NSFilePresenter/文件协调、编辑器集成 | 协作式在线操作的上下文和提示 | 没有发现能为任意外部编辑器停机期间替换提供普遍身份证明的公开保证 |

[Apple FSEvents Programming Guide](https://developer.apple.com/library/archive/documentation/Darwin/Conceptual/FSEvents_ProgGuide/UsingtheFSEventsFramework/UsingtheFSEventsFramework.html) 明确历史跨重启、可从保存的 event ID 读取；也要求将列表视为 advisory，遇到丢失必须全扫。SDK `FSEvents.h:307–344` 定义 `CreateWithDocID` 与扩展字典，但没有声明它是源文件身份追踪的写入替代。

此轮没有发现满足全部约束的普适原生方案，也没有把“未发现”宣称为数学上不可能。最实用的下一步是聚焦真实编辑器验收，准确界定可支持的原地保存行为；对于跨物理身份替换，保留明确的未完成状态，继续以独立正负样本验证候选机制。

## 037补充：实际WPS保存的合成文件

额外读取033原位保存合同及034通过WPS同格式另存的新副本，均为批准临时目录内的合成DOCX。复用本文相同SHA的探针、在新独占目录编译，两个全新进程共4次查询：Foundation成功但nil，底层返回属性且ID=0，指定stat字段未变；外围校验前后字节哈希相同。无flags/xattr setter，无Office新保存或安全替换操作。见[037原始结果](../validation/2026-11-20-wps-document-id-readonly-037.json)。

它补足先前只测空样本的限制，但仍未覆盖已有非零ID、Word、所有编辑器版本/保存模式或跨日替换；不改变当前不能普遍自动归并物理替换的结论。外围哈希读取合成正文仅用于验收原件不变，生产索引没有增加正文读取。
