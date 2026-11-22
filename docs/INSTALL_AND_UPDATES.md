# 安装、构建与应用内更新

本文件描述维护流程和信任边界，不代表所有平台的安装、真实编辑器保存或线上升级均已验收。可用平台与安装包以[官网下载页](https://worktwill.haoclub.com/zh/download)为准。

迭代范围、版本选择、变更日志、分平台发布状态和发布后跟进统一见[版本迭代流程](RELEASING.md)；本文件维护安装、打包、签名及更新的具体操作。

## 安装与信任

用户无需 Node.js、pnpm 或 Rust。双语离线说明的唯一来源是 [INSTALLATION.txt](INSTALLATION.txt)：发行脚本在安装包旁生成 `WorkTwill-INSTALL.txt`，官网构建复制同一文件供下载。在线说明由 `website/templates/guide.html` 和英文词典生成；步骤变更时同步维护。

当前分发方式为 macOS DMG 和 Windows 当前用户 NSIS 安装，不提供系统代码签名或 macOS 公证。核对官方来源、版本和 SHA-256，并遵循设备安全策略。开发者未验证提示与“损坏/将损坏电脑”分开处理，后者应停止并核对下载。不提供全局关闭 Gatekeeper/SmartScreen 的脚本，受管理设备由管理员处理。macOS 指引依据 [Apple 官方说明](https://support.apple.com/zh-cn/102445)。

应用内更新包独立签名并强制验签；它不等于 Apple Developer ID、Windows 代码签名或 SmartScreen 信任。

## 更新行为与状态归属

- 启动五秒后检查新版，此后每六小时检查；设置可以手动检查或关闭自动下载。关闭自动下载仍检查版本。
- Rust 后台下载、落盘和校验，准备完成后显示重启更新及纯文本更新说明；不强制重启。
- 安装前锁定交互，复用便签串行保存队列等待最新草稿确认。失败保留草稿和应用，不调用安装器；页面操作、表单及外拖准备期间不能安装。
- 更新后恢复语言和原工作区/主页面；搜索页返回首页，不承诺恢复未保存表单、查询、选择和滚动。
- 没有内置更新器的旧版本需要先手动安装一次。相同版本号重新打包不会触发已有同版本客户端更新。

`src-tauri/src/updates.rs` 拥有检查、缓存、下载偏好和安装状态；React 消费带 revision 的状态。业务 SQLite 和备份恢复仍归 Store，不由更新器维护第二套业务数据，也不提供任意数据库降级或整库自动回滚。

Tauri updater 验证签名及绑定的宣布版本，恢复缓存和安装时重新验签。缓存/偏好在应用数据目录 `updates/`；制品仅允许 HTTPS，缓存名绑定版本和签名，单包上限 512 MiB。连接或读取 30 秒无进展报错，最多三次尝试；支持续传和清单中的显式镜像。损坏缓存明确报错，更新错误不阻断本地业务，不静默登记成功。

## 密钥与原生构建

使用 `package.json`、锁文件和 `rust-toolchain.toml` 声明的工具链，在目标平台原生构建。现有签名公钥在 `src-tauri/tauri.conf.json`；本机私钥目录 `.release-keys/` 被 Git 忽略。不要读取或打印私钥到普通日志，也不要把它放进 Issue、PR 或制品。私钥需要私有离线备份。

另一台构建机/CI 使用同一密钥，通过 `TAURI_SIGNING_PRIVATE_KEY`（内容或文件路径）及 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` 提供。不要为修复构建重新生成或轮换密钥，否则已有客户端的信任链会中断。`pnpm release:keygen` 仅用于获得明确授权的首次初始化，不是日常构建步骤。

```sh
# 仅本地调试，不创建正式签名更新包。
pnpm tauri build --debug --bundles app

# 读取现有更新密钥，构建本平台安装包及签名更新包。
pnpm release:build
```

按[版本迭代流程](RELEASING.md)核对应用配置和两份 Rust crate 清单中的版本；发行脚本通过配置与locked Cargo metadata检查应用及两份crate版本一致，要求构建前后源码干净。macOS 输出 `target/release/bundle/dmg/` 中的 DMG 和 `macos/` 中的 `.app.tar.gz` 及 `.sig`；Windows 输出 `nsis/` 中的 EXE 及 `.sig`。离线安装说明随包附带。默认 CI 模式避免 Finder 排版自动化依赖；构建成功仍需要实际安装和更新验证。

## 登记与发布

**候选与公开记录隔离：** `release:record` 只生成独立候选目录，不修改 `website/release.json`；正式提升使用 `release:promote`，且要求同一份包的真实平台验收报告、递增版本和未变化的平台基线。提升只准备本地记录，不自动上线。

在干净源码提交上运行 `release:build`；脚本核对四处版本和Cargo锁文件，构建前后核对源码状态，并在安装包目录写入 `build-provenance.json`。不要将旧包与当前提交临时拼成来源记录。以下路径与版本仅作示例：

```sh
pnpm release:record --target darwin-aarch64 --version 1.3.2 --installer target/release/bundle/dmg/WorkTwill_1.3.2_aarch64.dmg --update target/release/bundle/macos/WorkTwill.app.tar.gz --provenance target/release/bundle/dmg/build-provenance.json --notes-file /path/to/release-notes.md --output /path/to/new-candidate-directory
pnpm release:verify --candidate /path/to/new-candidate-directory/candidate.json
```

候选目录包含精确制品、签名、SHA-256、安装说明、来源清单和未通过的验收模板。Windows目标为 `windows-x86_64`，安装和更新参数必须指向同一NSIS EXE；Intel Mac仅支持本地对应平台构建，没有云端工作流。验签复用客户端的minisign库，同时验证内容、全局签名和声明版本；不只检查注释。

跨机验收、验收后草稿归档与正式提升命令见[Agent验收交接](RELEASING.md#cross-machine-agent-acceptance)。没有对应平台桌面能力时，由维护者在Windows机器运行Agent接手，不得用CI打包替代。当前尚无用户，先使用真实官网更新地址跑通流程：发布前验证安装、启动、相关功能与许可；获准发布后再由旧版实际升级，确认数据保持。`release:accept --phase prepublish` 检查发布前条件，默认 `--phase complete` 要求升级和数据保持也通过。首次基线可先发布，但不能称升级已验收；升级演练需更高版本。未完成时保留“已发布、升级待验收”，不额外引入测试通道。

默认随静态官网分发小包；现有发行记录支持独立HTTPS地址，候选命令当前不自动接入外部存储；确需外部托管时先明确存储与上传方案。超过 25 MiB 的制品必须使用独立下载存储；R2 正式分发使用自定义域名。引入存储或变更域名另需明确任务授权。

发布顺序：通过检查 → 原生构建与必要验证 → 登记 → 上传并核对全部制品 → 最后发布官网和更新清单 → 核对线上下载与更新。制品长期缓存，更新清单重新验证缓存；旧制品不能因同版本重建被替换。

上述上传前后顺序按存储方式执行：外部托管先核对远端制品再公开入口；随官网打包的资源先核对构建内容，再与页面及更新清单一起部署，最后检查实际线上字节。已验证的包直接提升，不为正式发布重新打包。候选归档、正式版本留存及撤回要求统一见[版本迭代流程](RELEASING.md)。

本地构建/登记不等于上线。推送仓库、创建 GitHub Release、上传制品、部署官网和更新清单均按当前任务明确授权执行；既有明确授权不重复确认。官网命令与构建来源见 [website/README.md](../website/README.md)。

## GitHub Actions Windows 候选

工作流 [windows-build.yml](../.github/workflows/windows-build.yml) 仅通过 `workflow_dispatch` 手动运行，使用 `windows-2022` x64 MSVC。不因 push/PR 自动运行，生成隔离候选，不创建 GitHub Release、不提升公开记录、不部署官网。工作流须已在默认分支，才能从 Actions 页面选择分支运行。

维护者将现有密钥内容配置为仓库 Actions Secret `TAURI_SIGNING_PRIVATE_KEY`；有密码时配置 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`。Secrets 托管变更需要明确授权，不使用外部 PR 或不可信代码运行持有签名密钥的构建。密钥仅注入打包步骤。

工作流从锁文件安装，运行 `pnpm check`，再构建、密码学验签并登记隔离候选。Artifacts 保留 14 天，包含 EXE、`.exe.sig`、`WorkTwill-INSTALL.txt` 和 `SHA256SUMS.txt`；摘要标明版本与源码提交。下载候选后仍需 Windows 真机验证，之后才可登记并公开分发。

14 天的 Actions 制品是候选中转，不作为长期发行档案。另有手动 `macos-build.yml` 构建 Apple Silicon 候选。验收后 `release:archive` 可上传并回读核对GitHub草稿归档；公开Release与官网部署仍是独立授权操作，未实现统一自动发布。

## 验证与记录

两端分别执行与本次变更相关的安装 A → 检查/下载 B → 保存并重启 → 确认 B 版本和工作区、引用、收藏、便签保持。记录准确源码/制品、系统与编辑器版本、结果和已知限制。组件测试、缓存预置、本地网络夹具和 CI 打包分别标明，不能作为线上下载或完整安装升级通过。

系统提示、公证、SmartScreen、WebView2、权限目录、真实编辑器和恢复需要对应环境证据；未执行项如实保留。原始记录放在忽略的内部资料目录，公开发布说明提炼已验证范围和用户需要知道的限制。
