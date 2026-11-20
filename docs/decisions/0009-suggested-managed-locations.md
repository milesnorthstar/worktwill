# 0009：首次使用的常用位置建议

日期：2026-11-20。状态：迭代010实施；原生选择器起点和实际确认仍需真机验收。

PRD F01要求首次建议桌面、下载和文档。此前只有通用添加管理位置入口，未完整实现建议。现在首次动态页、无资料列表和无有效目录/单文件授权的设置页，复用同一受控LocationSuggestions组件；已有授权、读取未完成和搜索无结果时不展示首次建议。页面继续管理语言、busy和请求，不另设持久状态。

Rust DTO新增SuggestedLocation三值枚举，TS按既有工具生成。pick_root接收可空suggestion；null保持通用选择器，三个枚举由Rust的app.path()标准目录解析。解析错误明确返回可见错误。前端不传任意路径，不通过硬编码用户目录拼接路径，也不提前枚举或创建目标目录。

建议只传给原生dialog.set_directory设置起点，不直接调用grant_root。用户可在选择器中更换目录，只有blocking_pick_folder返回Some后才授权最终选中的目录并启动扫描；取消不授权、不emit、不kick_scan。取消后React的reload仅重读当前状态。所有入口使用当前语言，等待选择器时禁用其他首次入口，失败后可重新选择。

Tauri 2路径管理入口依据[官方迁移说明](https://github.com/tauri-apps/tauri-docs/blob/v2/src/content/docs/start/migrate/from-tauri-1.mdx)及当前安装的tauri/dialog/rfd源码核对；原生API本身不构成实际交互通过。独立只读复核确认了权限意图、消费者和本地macOS选择器实现，没有执行原生操作。

UI/API回归覆盖三值与null传输、取消不创建资料、语言切换、读取期间隐藏、目录/单文件已有授权隐藏、请求期间禁用及失败重试。Chrome正式App+mock IPC空库夹具检查中英文、100%/175%/200%下的动态、空资料列表和设置布局；夹具持续标识，不连接原生后端。具体检查与构建证据见[验收记录](../ACCEPTANCE.md)。
