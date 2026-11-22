# 官网安装与应用内更新

2026-11-21，依据用户明确同意的方案实现。本地工程能力、原生安装升级及线上服务分别验收。

## 当前分发决策

按用户确认，当前采用免费开源软件常见的分发方式：暂不购买 Apple Developer Program 或 Windows 代码签名服务，提供官网下载、版本与 SHA-256、系统首次打开指引，继续强制校验更新包签名。系统签名/公证是后续发行体验选项，不是本阶段前置；也不将未签名发行写成已完成系统签名验收。

双语离线安装说明来源为 `docs/INSTALLATION.txt`。`release:build` 在 DMG/NSIS 旁附带 `WorkTwill-INSTALL.txt`，官网构建复制同一文件供下载。官网 `/guide#macos`、`/guide#windows`、`/guide#updates` 提供对应步骤，应用设置中提供简短安装更新帮助。首次系统提示仍需用户确认，托管电脑的策略可能不允许；不修改系统保护设置，也不提供全局关闭 Gatekeeper/SmartScreen 的脚本。

macOS 的开发者未验证提示与“已损坏/将损坏电脑”分开处理，后者停止安装并核对下载。说明依据 [Apple 官方指引](https://support.apple.com/zh-cn/102445)。Windows 未提供允许选项时交由设备管理员处理，不承诺所有未签名包都能绕过策略。

## 用户行为

- 官网按平台展示已经登记的安装包；未发布的平台不给空链接。Windows采用当前用户NSIS安装，Mac采用DMG。
- 启动五秒后检查新版，随后每六小时检查；设置可手动检查或关闭自动下载。关闭下载仍检查新版。
- Rust后台下载、落盘和校验，不强制重启。就绪后显示“重启更新”和纯文本更新说明。
- 点击更新锁定交互，复用便签的串行保存队列，等待最新草稿确认。失败保留草稿与应用，不调用安装器；页面操作、表单、外拖准备期间不能安装。
- 更新后恢复语言和原工作区/主页面；搜索页返回首页，不承诺未保存表单、查询、选择和滚动恢复。应用identifier与正式数据目录不变。
- 没有内置更新器的旧版需先手动安装一次。

## 状态、数据与信任

`src-tauri/src/updates.rs`拥有检查、缓存、下载偏好和安装状态。React页面订阅带revision的状态，设置组件受控，DTO由model.rs生成。业务SQLite仍归Store，schema11及授权事实源不变，升级前快照复用原逻辑。更新器不提供数据库任意降级或整库自动回滚。

Tauri updater固定2.14.1，负责版本比较、验签、恢复缓存与安装。启用requireSignedVersion，签名绑定宣布的版本；恢复及安装重新验签。更新签名不等于Apple Developer ID或Windows代码签名。

缓存和偏好在应用数据目录updates/；隔离验收沿用WORKTWILL_VALIDATION_DATA_DIR。下载地址仅HTTPS，缓存名由版本和签名散列生成，单包最多512MiB。连接或读取30秒无进展报错，至多三次尝试，支持206续传及服务端忽略Range时从头下载。清单可显式提供mirrors数组，重试按序切换；所有来源仍由同一制品签名约束。

完整制品先验签，再写ready.json；离线检查保留已就绪版本。损坏缓存明确报错，隔离清单后下次重试。更新错误不阻断本地业务，不静默登记成功。

## 构建与发布

本轮已执行一次密钥生成。重复执行拒绝覆盖：

```sh
pnpm release:keygen
```

公钥进入tauri.conf.json。**私钥为项目内 `.release-keys/updater.key`，已被Git忽略，请离线备份。** 不打印、不提交、不公开上传。丢失后无法继续为内置此公钥的版本签发更新。

另一台构建机/CI可通过TAURI_SIGNING_PRIVATE_KEY（密钥内容或文件路径）和TAURI_SIGNING_PRIVATE_KEY_PASSWORD提供同一签名密钥，无需在另一端重新生成密钥。公钥随源码配置同步。

在相应平台原生构建：

```sh
pnpm release:build
```

要求package.json与Tauri版本一致。Mac输出target/release/bundle/dmg/中的DMG，以及macos/中的WorkTwill.app.tar.gz和.sig。Windows输出nsis/中的EXE和.sig，更新复用NSIS包，默认当前用户安装和quiet更新。当前不要求提供系统签名凭据；后续如启用 Apple 签名/公证或 Windows 代码签名，再由发布环境提供，更新密钥不会代替系统签名。

构建默认CI模式，避免依赖Finder排版自动化。首轮普通DMG构建曾因Finder AppleEvent超时失败，原始日志保留，不计通过。

登记已经核对的制品，例如：

```sh
pnpm release:record --target darwin-aarch64 --version 1.3.1 --installer target/release/bundle/dmg/WorkTwill_1.3.1_aarch64.dmg --update target/release/bundle/macos/WorkTwill.app.tar.gz
pnpm website:check
```

Windows使用windows-x86_64，两个文件参数都传NSIS安装EXE；Intel Mac使用darwin-x86_64。必须在对应构建和验收后登记，不借旧候选宣称新版支持。

当前本地完善包沿用1.3.1，尚未上线；同版本重新构建不能触发已有1.3.1的自动更新，需要手动安装。本轮新制品、检查结果和实际验证边界见[交付记录](validation/2026-11-21-unsigned-distribution.json)，旧交付记录与制品保留为历史。正式向已安装用户推送功能更新时，先递增package.json和tauri.conf.json的版本，再重新构建并登记。

website/release.json是逐平台发布记录的唯一来源。脚本复制到带版本和平台的固定文件名，记录大小、SHA-256、更新签名，拒绝覆盖内容不同的同名制品。官网核对制品并生成平台下载按钮及updates/stable/darwin-aarch64.json等清单；平台版本可以分别推进。

小包可随官网静态资源发布。R2分发时，登记增加`--base-url https://你的下载域名/固定版本目录/`。构建仍核对本地制品，但下载指向HTTPS地址，不重复打进站点。超过25MiB的制品必须走独立下载存储。正式R2使用自定义域名，不使用r2.dev。

先上传并核对全部制品，最后发布官网和更新清单。清单强制重新验证缓存，版本文件可长期缓存。本轮不修改DNS、创建R2或部署网站。

## GitHub Actions Windows 构建

工作流为 [Windows installer](../.github/workflows/windows-build.yml)，使用 `windows-2022` 的 x64 MSVC 环境。仅支持 `workflow_dispatch` 手动触发，不因 push/PR 自动运行，不创建 GitHub Release、不修改官网发布记录，也不部署官网。工作流须先合入默认分支，才能从仓库 Actions 页面手动选择分支运行；仅运行受信任的项目代码，不能用携带发行密钥的任务构建外部 PR。

首次运行前，在仓库 **Settings → Secrets and variables → Actions** 配置：

- `TAURI_SIGNING_PRIVATE_KEY`：已有 `.release-keys/updater.key` 的完整文件内容，不是本机路径。必须使用与 `src-tauri/tauri.conf.json` 内公钥对应的密钥；不要重新生成或轮换，不在 Issue、日志或源码中粘贴。
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`：该密钥的密码。当前无密码密钥可不配置此项，现有发行脚本按空密码处理。

上传私钥到 GitHub Secrets 属于密钥托管操作，需单独确认；工作流文件本身不执行密钥上传。签名值仅在打包步骤注入，不交给依赖安装和测试步骤，也不作为制品上传。更新签名不是 Windows 系统代码签名。

在 **Actions → Windows installer → Run workflow** 中选择经确认的分支运行。流程复用锁定依赖、仓库 Rust 工具链、`pnpm check` 和 `pnpm release:build`；测试样本放在 runner 独立临时目录。检查失败不继续打包，打包后核对安装程序、签名的版本声明和附带安装说明。

成功后，从该次运行的 **Artifacts** 下载 `WorkTwill-<版本>-windows-x64-<运行ID>-<重试次数>`，保留14天，包含：

- NSIS 安装程序 `.exe`；
- 对应 `.exe.sig` 更新签名；
- `WorkTwill-INSTALL.txt` 双语安装说明；
- `SHA256SUMS.txt`，记录上述三个文件的 SHA-256。

运行摘要记录版本与源码提交。流水线通过只说明检查和打包通过，不代表真实 Windows 安装、启动、保存或更新验收通过。必须在 Windows 电脑验收后，再使用现有 `release:record --target windows-x86_64` 登记同一个 EXE 和签名，按前述流程发布官网。配置工作流不表示已执行云端构建或已上线。

## 必需的原生验收

组件/API、Rust网络与签名回归、应用构建和官网检查分别记录。最终仍需两端分别执行：安装A→下载B→保存并重启→确认B版本以及资料、便签保持。系统签名、公证、SmartScreen、WebView2、受限安装目录和线上网络不能由本地测试代替。
