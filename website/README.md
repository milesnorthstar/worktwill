# WorkTwill 官网

2026-11-22 已部署本轮双语UI与SEO/GEO优化，代码提交 `9c909bd`，Cloudflare版本 `d95a6d16-6cf5-4d94-bfac-24c06be8b72d`。线上12页、新语言路径、旧英文路径301、GitHub与结构化关联、robots/llms/地图/分享图、双语404、当前Mac安装包大小和SHA-256及签名更新清单均已HTTP核对。未执行IndexNow提交；UA模拟不代表真实爬虫，安装重启更新、搜索收录与AI引用效果仍需独立验证。报告存于已忽略的 `.cloudflare/reports/geo-http-2026-11-22T00-06-42.261Z.json`。下文有日期的未部署说明为历史记录。

2026-11-21 安装更新增量：源码的release.json已改为逐平台发布记录，下载页和更新JSON由同一记录生成；本地已登记含更新器的1.3.1 Mac DMG及签名更新包。此轮没有部署，不能将本地新版本写成线上已更新。构建、密钥、R2分发与验收边界见[安装更新](../docs/INSTALL_AND_UPDATES.md)。

当前按用户确认暂不购买系统代码签名。首次打开步骤维护在 `templates/guide.html`，首页下载区和下载支持页提供入口；中英双语离线说明统一来源为 `../docs/INSTALLATION.txt`，构建时复制到 `/downloads/WorkTwill-INSTALL.txt`。应用更新包仍独立签名并强制验签。请勿将“说明已提供”写成“系统提示或线上安装已验收”。

当前官网：**https://worktwill.haoclub.com/**。2026-11-20 已发布至用户现有 Cloudflare 账户，自定义子域名由 Workers 托管。发布版本与线上验证见 [VALIDATION.md](VALIDATION.md)。

官网独立于 `desktop/`、Rust 和根目录保留原型。使用现有 Vite 工具构建静态 HTML/CSS/JS，Cloudflare Workers Static Assets 托管；不需要数据库或服务端业务逻辑。

首页沿用用户提供原型的布局、产品窗口与紫蓝视觉，并按 UI 审查优化：正文保持浅色连续背景，开源区不再全宽反色，深色保留在首屏、下载主卡与页脚；正文无脚本时默认可见；脚本就绪后在进入视口时播放一次 0.95 秒淡入上移，文字与示意卡错峰 0.12 秒，准备态保留淡底避免整块空白；首屏保留光轨旋转与资料卡浮动，触屏同样生效，遵循系统减少动态效果设置。不恢复追光及鼠标跟随透视。导航提前折叠且语言入口常显，FAQ 按真实内容展开。搜索演示统一初始与输入后的结果，支持方向键、Enter、Esc 和点击；只反馈合成示例，不打开用户文件。首页保留的原型元素包括：深蓝渐变首屏、紫蓝高光、悬浮资料卡、原型产品窗口、浅色功能区、工作区与动态示意和双平台下载卡片。不混入旧官网的产品演示、下载区或全局样式；`src/home.css` 和 `src/home.js` 独立于内页。继续使用语言切换、静态双语生成和真实发行记录，版本、下载、开源入口及联网说明以项目事实为准。首页动态文案复用英文词典并生成 `src/home.generated.js`，不手改生成文件。导航只保留 GitHub 图标直达 https://github.com/milesnorthstar/worktwill，桌面与窄屏均可见，不设置开源计划/进展导航或独立宣传区；说明、隐私与下载支持链接留在页脚，不插入改变原型布局的资料导航块。页面中的资料均为合成示例，演示不连接桌面端或读取本地文件。

英文为默认入口 `/`、`/guide`、`/privacy`；中文使用 `/zh/`、`/zh/guide`、`/zh/privacy`。语言入口按 EN、中文排列，旧 `/en/` 系列地址在部署时通过静态 301 跳转到对应英文地址；canonical、x-default、站点地图及 llms 导航同步使用新映射。顶部中英文链接切换到对应页面；不按IP或浏览器语言强制跳转，不使用localStorage记录另一份语言事实。

内页（产品事实、合同场景、下载、使用说明、隐私与404）通过构建期复用首页品牌导航和页脚，`src/document.css` 引用 `home.css` 的设计变量及基础组件，使用深蓝渐变页头、浅色正文、紫蓝按钮与卡片。内页窄屏导航直接换行，不加载首页搜索演示脚本，也不依赖菜单展开才能访问。

SEO/GEO 本地回归包含发布阶段与版本一致性、页面日期/JSON-LD/地图日期一致性、GitHub实体关联及过时文案拦截。HTTP审计回归使用 `node --test website/scripts/audit-geo.test.mjs`；实际线上审计按时间戳保存报告至已忽略的 `website/.cloudflare/reports/`，不覆盖历史。模拟UA只验证该请求的可读性，不代表真实爬虫来访或AI引用。

## 内容来源与SEO

`templates/` 是页面结构及中文文案来源，`locales/en.json` 是完整英文词典，`locales/demo.mjs` 是演示数据和动态状态文案来源。`site.config.mjs` 维护正式origin和路由。`scripts/pages.mjs` 在构建前生成8个完整静态HTML及精简演示字典，缺少英文翻译时直接失败。

请编辑上述来源，不手改根目录或 `en/` 中生成的HTML及 `src/demo.generated.js`。当前复用项目已有jsdom做构建期DOM变换；两种语言共用一个模板，不维护两套结构。网站依旧是Vite多页静态站，没有引入Next.js或客户端整页翻译。产品正文、FAQ、首页示例及下载在禁用JavaScript时仍完整存在。

12个可索引页面各有独立title/description、self canonical、`zh-CN`/`en`/`x-default`互相回指的hreflang、Open Graph/Twitter元数据和1200×630双语PNG分享图。站点地图包含12个URL及对应alternate。JSON-LD包括WebSite/WebPage、产品页面SoftwareApplication、说明页BreadcrumbList；不虚构评分、价格或许可证。404设置noindex，不进站点地图。

分享图文件在 `public/og/`，样式更新时可在macOS执行 `swift website/scripts/social-card.swift website/public/og` 重生成；正常构建不需要Swift。所有图、字体及图标仍同源加载。

`pnpm website:check` 会构建并验证原始HTML的正文、语言、canonical、hreflang互链、站点地图、PNG尺寸、JSON-LD及链接，发布命令先通过这项检查。Google Search Console已验证并成功处理地图、发现12页；Bing已验证并提交地图，正在Processing；IndexNow返回202。百度按用户明确指示不纳入本轮。发现URL不等于已收录或有排名。

GEO新增产品事实 `/about`、客户合同步骤 `/contract-workspace`、独立下载支持页 `/download`，均有英文对应、静态导航和核对日期。双语页面已接入公开源码仓库，许可证与贡献说明以仓库内容为准，不自行宣称具体许可证。执行依据、基线、固定问题与后续观测方法见 [geo/README.md](geo/README.md)。

## 本地与构建

在仓库根目录执行：

```sh
pnpm website:dev
pnpm website:build
pnpm website:check
pnpm website:preview
```

预览地址 `http://127.0.0.1:4321/`，生产输出只在 `website/dist/`，不覆盖桌面端 `dist/`。

构建前读取 `release.json`，核对 `deliverables/` 中已记录候选包的大小和 SHA-256，复制到官网自己的下载目录。制品缺失或不匹配立即失败，不静默发其他版本。更新发行时同步 `release.json`、页面中的版本/平台文案和对应使用说明。`deliverables/` 由现有规则忽略，干净环境需要另行提供经核验制品，不会从 GitHub 自动下载。

官网本身及下载资源均同源提供，不依赖 Google Fonts、GitHub、外部图标 CDN 或统计脚本。SVG 图标使用本地 `lucide` 0.468.0（ISC），仅打包使用的图标。Cloudflare CLI 固定为官方 `cf` 1.0.0-rc.2；当前官方文档针对 RC，npm 稳定标签仍可能指向旧同名包，不要改成无版本安装。

## Cloudflare 发布

```sh
# 在根目录登录；不要把凭据写入源码
pnpm exec cf auth login
pnpm exec cf auth whoami

# 构建并检查配置，不上传
pnpm website:deploy:check

# 由已登录账户发布官网与下载文件
pnpm website:deploy
```

`cloudflare.config.ts` 只在官网目录生效。Worker 名称 `worktwill-website`；安装包当前约 6.6 MB，随静态资源发布。当前没有安装更多版本的目录；以后确有大文件、多版本分发需求再引入 R2。

首次登录后，先确认当前账户及 Worker 同名资源，再部署，避免覆盖无关站点。多账户时明确选择账户，不自动推测。已有同名站点时先查看状态并确认归属。

已按用户授权使用现有主域名 `haoclub.com` 下的 `worktwill.haoclub.com`，发布前核对该DNS名称与Worker名称未占用。配置中的 `accountId` 固定为本项目已核实的账户；它是公开资源标识，不是凭据。Cloudflare已自动创建该子域名的代理DNS与HTTPS服务；未修改主域名或其他应用的路由。canonical、Open Graph URL、双语分享图及站点地图均使用正式地址。

中国大陆访问仍需从大陆网络单独验证首页、说明页与安装包下载；不能用当前网络或Cloudflare默认域名的结果宣称大陆可用。

## 验证边界

网站验证包括生产构建、交互演示、桌面和窄屏排版、站内链接、下载包完整性与发布 dry run。它不代替桌面原生窗口、Office 保存、Windows、Apple 签名和公证等产品验收。

线上Chrome访问、curl HTTP/响应头与安装包完整性检查已执行；中国大陆网络尚未独立实测。详见 [线上HTTP证据](../docs/validation/website/live-http-2026-11-20.json)。
