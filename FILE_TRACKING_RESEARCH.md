# 文件跟踪、办公保存与竞品调研

日期：2026-11-19。范围：WorkTwill 已有文档、Tabbles 公开资料、TagStudio v9.6.4 文档和源码、TagSpaces 官方文档，以及 Apple/Microsoft 原生 API 文档。

版本说明：本文记录调研时的 v0.4 基线，表中行号也对应该版本。后续 PRD/ARCHITECTURE v0.5 已收录原生文档身份的候选策略与验证要求；本文的“建议/尚未改写”描述保留调研当时状态，不表示相关原生实验已经完成。

本次为资料与源码调研，没有安装竞品或操作真实 Office/WPS，没有执行原生保存、改名和移动实验。不将厂商宣传、源码推断或 API 契约等同于实际软件验收。

## 1. 先核对现有方案

**用户记得正确：技术文档已经有原生跟踪方案。**

| 已有设计 | 文档位置 | 目的 |
|---|---|---|
| Windows `FILE_ID_INFO`，卷序列号 + File ID | ARCHITECTURE §4，154 行起 | 识别物理文件 |
| `OpenFileById` + `GetFinalPathNameByHandle` | ARCHITECTURE §4，155 行起 | 找回同卷改名或移动后的文件位置 |
| macOS `NSURL` Bookmark，含持久权限验证 | ARCHITECTURE §4，163 行起 | 持久化定位和访问凭据 |
| `ReadDirectoryChangesW` / `FSEvents` | ARCHITECTURE §5，187 行起 | 观察变化并核对目录 |
| S01/S02 中的 Word/WPS 普通保存与安全替换 | ARCHITECTURE §8，259 行起 | 验证实际恢复范围 |

上一轮将“日常保存可能打断引用”突出为风险，容易给人“没有考虑原生跟踪”的印象，表述需要收窄。准确结论是：**已有合理的定位和监听方案；还应补充原生文档身份候选，并完成已经安排的保存实验。**

## 2. 竞品实际采用了哪些取舍

### Tabbles：Explorer 集成、数据库关系与失联修复并存

**查到的事实**：

- 当前官网宣传：标签存数据库，不必修改文件名或生成源目录隐藏文件；通过 Windows Explorer 移动、改名时，标签跟随。
- 官方手册说明，自 v3 起跟踪 Explorer 中的文件/目录移动、改名、删除及剪切粘贴。
- 同一手册保留 Shadow Files：未被 Tabbles 观察到的移动/删除会导致失联，可让用户指定新位置。举例包括网络上的其他机器，以及没有使用对应集成入口的其他文件管理器。
- 手册和更新日志也记录了向 Office 文档元数据写标签的能力，并提供不写入的选项。不能把这类携带标签的方案当作严格只读数据库跟踪来比较。

**不能据此确认的事**：公开资料没有充分披露它在所有版本中的底层身份算法，也没有证明任意 Word/WPS 安全保存、程序未运行期间变化、任意文件管理器操作都能无条件恢复。手册包含较老版本内容，不能用它直接断言当前安装包仍存在某个历史缺陷。

**对我们的启发**：关系放数据库、跟踪尽力而为、失联保留并允许修复是成熟产品也采用的路线。Shell 集成是另一项工程与发行成本，不应仅因对方有此能力就加入我们的 V1。

来源：[官网](https://tabbles.net/)、[官方手册](https://tabbles.net/wiki/index.php?title=Tabbles_Manual_(EN))、[更新日志](https://tabbles.net/en/changelog/)。

### TagStudio v9.6.4：按路径保留条目，同名搜索修复移动

查询到最新正式发行标签为 `v9.6.4`，发布于 2026-10-02。本次关键结论核对了该发行标签下的文档和源码，而非仅看开发分支。

**查到的事实**：

- 使用文档要求通过 Manage Unlinked Entries 刷新失联条目，再 Search & Relink。
- 文档明确警告：当前不能恢复已改名文件；移动文件的匹配依据是原文件名，多候选时忽略该组。
- `UnlinkedRegistry.refresh_unlinked_files` 以原路径是否存在、是否为文件判断失联。
- `match_unlinked_file_entry` 在库目录内按原文件名搜索；`fix_unlinked_entries` 仅在一个匹配时更新路径，必要时合并条目。这段流程没有使用 Windows File ID 或 macOS Bookmark 作身份判断。

**有限推断**：因为此流程按路径是否存在判定，原路径仍存在时，底层对象变化本身不会触发这条失联路径。这可能减少同路径保存后的修复提示，也意味着其依据与我们“防止同路径新文件承接旧关联”的严格规则不同。这不是对全部保存行为的运行时证明。

**对我们的启发**：它不是已经实现了更强的原生身份追踪。失联管理可以参考；按同名唯一候选自动绑定的规则不符合我们当前的身份约束，不能直接照搬。

来源：[v9.6.4 发行](https://github.com/TagStudioDev/TagStudio/releases/tag/v9.6.4)、[该版本使用文档](https://github.com/TagStudioDev/TagStudio/blob/v9.6.4/docs/usage.md#relinking-moved-files)、[该版本 UnlinkedRegistry 源码](https://github.com/TagStudioDev/TagStudio/blob/v9.6.4/src/tagstudio/core/library/alchemy/registries/unlinked_registry.py)。

### TagSpaces：让组织信息跟文件走，但会改变文件名或添加 sidecar

**查到的事实**：官方提供两种文件标签存储方式：将标签写进文件名，或在源目录的隐藏 `.ts` 文件夹中保存 JSON sidecar。官方也说明，在应用外移动或改名时，sidecar 需要相应维护。

**对我们的启发**：这是可移植性与源目录改动之间的另一种取舍。两种方式都与 WorkTwill 当前“不更名、不创建源目录 sidecar”的原则冲突，不能作为现有架构的直接替代。

来源：[TagSpaces 官方标签文档](https://docs.tagspaces.org/tagging/)。

### 对比结果

| 产品 | 已核实的关系/恢复方式 | 与 WorkTwill 的主要差异 |
|---|---|---|
| Tabbles | 数据库、Explorer 集成、Shadow Files 修复；可选 Office 元数据 | 集成范围有边界，部分能力会写元数据；未证实全部保存场景 |
| TagStudio v9.6.4 | 路径存在性检查、库内同名搜索，唯一候选重新链接 | 对同路径对象替换与同名匹配采取不同的身份取舍 |
| TagSpaces | 文件名携带标签或源目录 sidecar | 不满足我们的原件与源目录约束 |
| WorkTwill 设计 | UUID 关系、原生 locator、授权与监听、人工修复 | 方向合理；原生文档延续身份与真实保存效果待补验证 |

## 3. 新核实的原生能力

### macOS：`documentIdentifierKey` 与 Bookmark 应组合评估

Apple 官方明确说明 `URLResourceKey.documentIdentifierKey`：

- 文档标识由内核分配，标识文件或目录；同卷移动仍可保持。
- 跨系统重启持久存在；复制文件时不转移给副本。
- 能延续 safe-save，例如调用 `FileManager.replaceItem` 后仍保持在被赋予的路径上。
- 只在同一卷内唯一，并非所有卷支持。

这与单纯的 inode/File ID 不同，直接针对“用户仍在编辑同一份文档、底层文件发生替换”的需求。

**建议候选组合**：Bookmark 负责解析位置及适用时的访问凭据，文档标识负责补充文档延续证据，FSEvents 负责观察和补扫。它们职责不同，不能仅保存一个 document ID 就认为已经具备定位和授权能力。

另需避免把 `fileResourceIdentifierKey` 直接当长期持久标识；Apple 对这个属性明确说明其值不保证跨系统重启持久。

**尚不能承诺**：该能力不证明任意 Word/WPS 或同步客户端都使用了能保持文档标识的保存方式，也不保证跨卷和所有云盘表现。Bookmark 是否已经对某些替换保存正确恢复，也应实测，不能先假设现有方案必然失败。

来源：[documentIdentifierKey](https://developer.apple.com/documentation/foundation/urlresourcekey/documentidentifierkey)、[fileResourceIdentifierKey](https://developer.apple.com/documentation/foundation/urlresourcekey/fileresourceidentifierkey)、[Bookmark 解析](https://developer.apple.com/documentation/foundation/nsurl/urlbyresolvingbookmarkdata:options:relativetourl:bookmarkdataisstale:error:)。

### Windows：必须区分 File ID 与 Object ID

| 原生能力 | 可确认的作用 | 限制或与现有约束的关系 |
|---|---|---|
| File ID + 卷身份 | 识别物理文件，同卷改名/移动时用于重新定位 | 不应视为所有保存后不变的文档标识 |
| `ReadDirectoryChangesW` | 观察目录变化 | 不是独立的持久文档身份；未运行或溢出时需核对 |
| NTFS Object ID | 可选的对象属性，用于原生链接跟踪 | 文件不一定已有；不等于 FILE_ID_INFO |
| `ReplaceFile` | 替换保存时保留原对象的 Object ID 等属性 | 结果采用替换文件的 File ID；实际编辑器是否使用该路径待验证 |
| `FSCTL_GET_OBJECT_ID` | 查询已有 Object ID；没有则返回错误，不创建 | 可作为候选附加证据，不能假设普遍存在 |
| `FSCTL_CREATE_OR_GET_OBJECT_ID` | 没有 Object ID 时创建一个 | 会设置源文件元数据，不符合目前不修改元数据的原则 |
| Distributed Link Tracking / Shell Link | 跟踪快捷方式、OLE 链接等；部分恢复含启发式搜索 | 非无条件身份保证，也不能未经评估直接作为只读通用定位器 |

关键证据：Microsoft `ReplaceFile` 文档同时写明保留原文件的 **Object identifier**，以及结果具有替换文件的 **file ID**。因此，“安全保存可能改变 File ID”不能推导为“Windows 没有原生延续机制”。

**建议**：继续以现有 File ID/定位/监听为基础，评估只查询已有 Object ID 的收益。不要为了提高恢复率自动给所有文件创建 Object ID，也不要悄悄改成写入 ADS、标签或隐藏文件。

来源：[ReplaceFile](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-replacefilew)、[Object IDs 与 Distributed Link Tracking](https://learn.microsoft.com/en-us/windows/win32/fileio/distributed-link-tracking-and-object-identifiers)、[查询 Object ID](https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ni-winioctl-fsctl_get_object_id)、[创建或查询 Object ID](https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ni-winioctl-fsctl_create_or_get_object_id)。

## 4. 对当前架构的建议

**保留架构和状态归属，扩充现有平台 locator 的证据与解析策略。** 不引入独立的“Office 文件关系系统”，不把工作区关系改绑到路径，不因本次调研默认引入 Shell 扩展或后台常驻。

现有路径可继续为：

```text
应用 Resource UUID（工作区/收藏关系）
    → FileLocator + 平台适配器（位置与身份解析）
    → 当前授权校验
    → 确认证据足够后更新缓存定位，或保留失联并提示
```

候选定位封装应区分：物理文件身份、原生文档延续身份、位置和平台访问凭据。具体字段与索引在 Spike 后决定，避免把平台 BLOB 当成无法比较的永久黑盒。

需要明确调整的一条规则是：**裸 File ID 变化不应单独触发人工修复；应先检查已选择并验证过的原生文档身份机制。** 这是一项建议，现有 PRD/架构尚未被本次调研自动改写。身份不明的同路径对象仍不能直接承接旧关联。

原生标识表达文件系统提供的身份延续，不会判断用户业务上的“是不是同一份合同”。明确的另存为、复制和替换操作仍需按照经产品批准的语义验收。

## 5. 把已有 S01/S02 做成可判定的实验

对每个场景，同时记录：File ID/Bookmark 恢复结果、可用的文档标识或 Object ID、实际路径、应用 UUID、工作区关系、权限和能否打开正确目标。

| 场景 | 必须检查的结果 |
|---|---|
| 普通保存、连续保存 | UUID 和关系保持；正确文档可直接打开；记录是否需要手动操作 |
| 编辑器自己的安全保存 | 比较物理身份与原生文档身份；不以 File ID 变化直接判失败 |
| 应用打开/关闭期间保存，随后重启 | 区分监听收益与原生持久恢复收益 |
| 同卷改名、移动，包含父目录改名 | 定位正确、UUID 不分裂、授权重新核对 |
| 另存为、复制 | 原记录与新副本按产品规则区分，不因相同内容合并 |
| 删除后同名重建、故意替换其他文件 | 不能仅凭同路径或同名自动绑定；记录原生标识实际变化 |
| 跨卷、云同步/占位符、外置盘 | 分别记录能力，无法恢复时保留关系和明确状态 |

先在目标本地文件系统验证 Word/WPS 的普通核心流程，再扩大同步目录和文件系统矩阵。人工指定的临时目录和测试文档用于可变更实验，不拿真实用户资料做改名、替换、删除。

**最终判断**：已有方案具有继续验证和实现的基础。竞品也存在范围限制与不同身份取舍；本次没有证据支持把它定性为不可行。新增证据支持补充“原生文档身份”这一层，尤其是 macOS `documentIdentifierKey`；Windows 则需在只读约束下评估已有 Object ID 与实际保存行为。
