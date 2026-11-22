# WorkTwill 技术实验

这些是与产品工程隔离的 Spike，不是正式 Rust Core 或 Tauri 应用。脚本和复现方法公开；原始实验报告、机器路径和截图保存在忽略目录，不代表产品已完成相应验证。

## S03：内存中的 SQLite 查询内核

运行环境要求 Python 3.10+，其 SQLite 必须支持 FTS5 与 trigram。只生成内存中的合成记录；`--output` 写实验报告，不创建真实文件样本或读取办公文档。

```sh
python3 spikes/search_kernel.py --output spikes/results/search-kernel-NEW-RUN.json
```

默认规模 10k/50k/100k/500k，每种策略、每个规模采样 570 次。复查时使用新的输出文件名，保留已有原始证据。

- 基线 `scan_short`：1–2 字查询用元数据扫描，3 字以上使用 FTS5 trigram。
- 候选 `short_grams`：1–2 字查询使用应用预生成的 unigram/bigram ASCII token，再由 SQLite FTS5 检索；3 字以上仍为 trigram。
- 候选索引并非 SQLite 已内置的中文 unigram/bigram tokenizer，也不是全文内容分词。两种策略都保留字面量核对、权限/类型/工作区过滤与相同排序。
- 归一化只为实验选择 NFC + casefold，不据此合并资源。工作区多对多和有效授权是合成投影，不是正式权限服务。
- 每个规模先与独立全量参照比较 19 类查询的 Top-10，再检查精确名优先、未授权重复项及静态撤权过滤。该检查不证明真实并发撤权正确。
- 两种策略交错采样并记录原始耗时、查询计划、构建时间和 SQLite 分配页体积。分配页含空闲页，不代表独立索引净体积或磁盘/WAL 体积。

该实验不覆盖 Rust 包装、磁盘冷热、并发写入、取消、分页、分组结果、Tauri IPC、输入法、快捷键和可见渲染；不能将 SQL 内核结果登记为 S03 全部通过。

### 磁盘 WAL 与并发写入

在下述 S02 脚本已创建、且用户确认过的样本根目录内运行：

```sh
python3 spikes/search_disk.py --fixture-root /absolute/path/to/approved-fixture-root --output spikes/results/search-disk-NEW-RUN.json
```

路径必须已获当前环境的明确授权。程序要求存在样本标记，并新建 `search-disk` 子目录；子目录已存在时拒绝覆盖。默认 10 万合成资源，先构建和核对索引，再备份至磁盘，使用 WAL 测量无写入与并发小事务写入的查询。停止写入后按当前业务行重新核对结果，不用最初静态数据冒充写入后的参照。

该程序不控制 WPS、不修改真实文件，不是 Rust 后端，也没有清空 OS 缓存；结果不能标为冷查询或完整 S03 通过。

## S02：macOS 原生身份探测

需要 macOS 与 Xcode/Command Line Tools。使用系统 Foundation/CryptoKit，不包含正式应用的 sandbox entitlement 或发布签名。

```sh
mkdir -p spikes/.build
swiftc -module-cache-path spikes/.build/modules spikes/macos_identity.swift -o spikes/.build/macos-identity
spikes/.build/macos-identity --inspect ARCHITECTURE.md
```

`--inspect` 只读元数据，在内存中创建并解析 Bookmark，不读取文档正文、不改名或替换文件。分别记录创建 Bookmark 前后及刷新 URL 缓存后的 Document ID 可用性。退出成功只表示探测完成，不表示追踪或办公保存验收通过。

### 文件变更实验

遵守 [开发约定](../AGENTS.md#验证与完成条件)：先确认一个专用、尚不存在的临时子目录，才执行 `--fixture-root`。脚本拒绝普通工作目录、临时目录本身和已存在目录；不会清空目录来满足条件。

人工确认路径后，将下面的 `NEW_TEMP_PATH` 替换为该路径：

```sh
spikes/.build/macos-identity --fixture-root NEW_TEMP_PATH
spikes/.build/macos-identity --resume NEW_TEMP_PATH/manifest.json
```

内建案例为原地写入、改名、同卷移动、Data 原子写入、FileManager 替换、复制、删除后同名重建。所有可变更目标均由本次运行在该目录新建，程序不接受任意待修改文件路径，不自动清理证据。

第一条命令记录变更前后标识、Bookmark 解析、内容摘要和错误，并保存重启解析用 manifest；第二条命令从新进程读取 manifest，检查 Bookmark 路径与内容。案例有错误也会记录结果，因此退出码不能代替逐项判定。nil Document ID 不算身份保持成功，Bookmark 返回路径也不自动证明指向正确业务文档。

以上仅是原生 API 操作，不能代替 Word/WPS 的实际保存行为、系统重启、授权撤销、跨卷或正式签名包测试。实验脚本不实现正式资源 UUID 与工作区关系，此层也不能记作产品已通过。

## 结果与验证边界

`spikes/results/` 被 Git 忽略，输出使用每轮独立文件名，不覆盖旧报告；需要共享结果时先脱敏并提炼结论。macOS Foundation、Windows NTFS、真实 WPS/Word、应用重启和正式安装包需要各自环境观察，不能用 SQLite 或 Swift 实验替代。
