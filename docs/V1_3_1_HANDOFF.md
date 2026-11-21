# V1.3.1 关机暂停与续接记录

2026-11-21，用户要求先暂停关机，随后明确授权本地 Git 提交。目标暂停，不表示 Mac 完整验收完成；不推送、合并、发布或部署。

## 已完成

- 已安全更新到基线 `9124cc6`，保留原有草稿；正式运行时代码和配套文档已完成本轮实现。
- 删除所有文件入口、固定收藏、首页真实最近打开、搜索状态隔离与80ms防抖、单击打开及重复事件防重、普通打开后台更新、系统工件去噪。
- 独立静态伙伴和 Rust/SQLite 正式便签；schema9、版本/墓碑/恢复scope、独立更新通道及升级前备份。
- 既有用户库工作区、关系和收藏升级保留。业务原件未修改。早期实际库验收便签的情况见 `V1_3_1_MACOS.md`，不要擅自清除。
- 上轮完整检查151项 UI/API、237项 Rust通过，6项规模基准显式ignored；已构建并启动核对 release 候选。
- 本轮追加修复：退出等待已在途便签保存，并继续提交其后的最新草稿；新增对应测试，UI总数152项。当前源码的完整提交前检查记录在 `validation/2026-11-21-v131-precommit-check.log`。

## 包与源码的区别

现有可运行包：

`/Users/lch/Desktop/project/my/test/deliverables/WorkTwill-V1.3.1-macOS-arm64-local/WorkTwill.app`

ZIP为同名目录加 `.zip`。**该release包不含本轮最后追加的“等待在途便签保存退出”修复**，其manifest指纹只对应当时构建，不代表暂停时全部源码。`target/debug/bundle/macos/WorkTwill.app` 已构建该修复，但不作为最终release交付。暂停前 WorkTwill 验收进程已退出。

## 尚未完成，恢复后继续

1. 核对Git状态及用户新改动，读取本记录与 `V1_3_1_MACOS.md`。
2. 完成可独立执行的原生后页/滚动、便签置顶/删除/全部、无工作区首批资料取回验证。
3. 真正前台可见的搜索P50/P95/P99、聚焦时延及中文输入法候选行为；此前后台AX采样 `document.hidden=true`，不能写为前台通过，也不能放宽预算。
4. Finder真实拖放与混合授权/取消、默认编辑器内容与视觉闪动、Office保存、同路径身份变化、完整多尺寸/200%中英文、原生SQLite中断直接证据。TextEdit控制曾被环境拒绝，不能换技术绕过或无限重试。
5. 无新增源码问题后，一次release重建，更新本地包、指纹和交付文档，明确替代旧包。当前不因关机要求重建。
6. Mac必做项未完成前不将目标标complete。Windows V1.3.1独立未验；旧版Windows结果不能当新版通过。

证据分层：Core/组件/合成IPC与原生观察分别见 `validation/2026-11-21-v131-native-acceptance.json` 和本地交付说明；已有通过分支不得扩大到未执行矩阵。

## 本轮独立夹具

`/private/tmp/worktwill-spikes-20261119/v131-isolated-native-4qPDRM`

其中 `app/` 是独立SQLite数据，`documents/` 是合成文件。110份真实合成文件加99890条仅索引干扰项，共100000索引；不是10万真实扫描证明。本轮已准备Finder客户甲窗口，尚未执行真实拖放、后页及新增便签操作。临时目录可能被系统清理；恢复时先检查，不清空现存目录。

debug重启命令：

```sh
open -n \
  --env WORKTWILL_TEST_ROOT=/private/tmp/worktwill-spikes-20261119 \
  --env WORKTWILL_NATIVE_DATA_DIR=/private/tmp/worktwill-spikes-20261119/v131-isolated-native-4qPDRM/app \
  --env WORKTWILL_NATIVE_MINIMUM=1 \
  /Users/lch/Desktop/project/my/test/target/debug/bundle/macos/WorkTwill.app
```

测试开关仅debug生效。正式用户库在 `~/Library/Application Support/app.worktwill.desktop/worktwill.sqlite3`，不能将业务文件当样本。

恢复口令：“继续 WorkTwill，按 docs/V1_3_1_HANDOFF.md 接着做”。继续授权只恢复开发与验收；此次Git授权仅本地提交，不包括推送。
