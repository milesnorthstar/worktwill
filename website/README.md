# WorkTwill 官网工程

官网独立于桌面运行时，使用现有 Vite 构建静态 HTML/CSS/JS，并由 Cloudflare Workers Static Assets 托管。正式 origin 与路由来自 `site.config.mjs`；下载及更新记录来自 `release.json`。

本文件维护页面来源、构建和部署；产品事实见[业务约定](../docs/PRODUCT.md)，随版本同步的步骤见[迭代流程](../docs/RELEASING.md)，持续内容核查与效果观测见[SEO/GEO 维护](geo/README.md)。

## 内容和生成来源

- `templates/`：共享结构及中文文案；`locales/en.json`：英文词典；`locales/demo.mjs`：演示资料和动态状态文案。
- `scripts/pages.mjs`：生成双语页面和精简演示字典。缺少英文翻译直接失败，不维护两套页面结构。
- `src/home.css`、`src/home.js`：首页布局和演示；`src/document.css`：复用首页变量及基础组件的内页样式。
- `../docs/INSTALLATION.txt`：离线安装说明来源，构建时复制到下载目录。
- `release.json`：逐平台版本、阶段、制品摘要、签名与下载地址的唯一来源。登记与更新流程见 [安装更新](../docs/INSTALL_AND_UPDATES.md)。

不要手改生成的根目录/语言目录 HTML、`src/demo.generated.js`、`src/home.generated.js`、地图、llms 或下载/更新输出；从模板、词典、站点配置和发行记录修改后重建。演示资料是合成示例，不连接桌面端或读取本地文件。

英文默认入口 `/`，中文 `/zh/`；语言切换指向对应页面，旧 `/en/` 路径生成静态 301。canonical、hreflang、x-default、地图与 llms 导航使用同一路由来源，不按 IP/浏览器语言强制跳转，不用 localStorage 保存另一份语言事实。

正文、下载、FAQ 和导航在禁用 JavaScript 时仍可访问。首页保留已有原型布局及动画，并遵循系统减少动态效果设置；内页不加载首页搜索演示。窄屏保留可访问的语言和 GitHub 入口。

## 本地与检查

在仓库根目录运行：

```sh
pnpm website:dev
pnpm website:build
pnpm website:check
pnpm website:preview
node --test website/scripts/audit-geo.test.mjs
```

预览为 `http://127.0.0.1:4321/`，输出仅在 `website/dist/`，不覆盖桌面 `dist/`。构建核对 `deliverables/` 中已登记安装包的大小和 SHA-256；缺失或不符直接失败。该目录被 Git 忽略，干净克隆需要另行提供发行记录对应的经核验制品，不自动下载，也不通过删除发行记录绕过核验。

纯页面、SEO/GEO 和说明更新使用当前已发布的逐平台记录及同一份包，不提升桌面版本。部署前核对登记与线上发行状态；目前 `release:record` 会直接修改生成正式更新清单的来源，“本地候选”文案不会阻止发布。未来干净 runner 将按摘要恢复已发布制品，候选隔离、恢复和正式提升自动化仍待实现，详见[发布模型](../docs/RELEASING.md#publication-model-and-current-implementation)。

`website:check` 验证页面正文、语言、SEO 元数据互链、JSON-LD、地图、分享图、站内链接、版本与阶段一致性；HTTP 审计回归隔离网络及写入。网站检查不替代桌面原生、Office 保存、安装或更新验证。

图标、字体、图片和下载默认同源，不引入外部统计脚本。SVG 图标使用锁定的本地 lucide；分享图可按需用 `swift website/scripts/social-card.swift website/public/og` 重生成，普通构建不需要 Swift。不要因更新文档或构建而机械刷新页面核对日期。

## 部署与线上观测

Cloudflare CLI 版本由项目依赖锁定；不使用无版本安装替换。Worker 和账户以 `cloudflare.config.ts` 为准。首次或切换环境时核对登录账户与资源归属，不覆盖同名无关站点，不打印凭据。

```sh
pnpm exec cf auth whoami
pnpm website:deploy:check
# 获得当前任务发布授权后执行。
pnpm website:deploy
# 只读检查线上公开内容及制品完整性；会联网并写本地报告。
pnpm website:geo:audit
```

登录、部署、DNS/存储变更和向搜索引擎主动提交 URL 分别按任务授权执行。先上传并核对制品，最后部署更新清单，不能把构建或 dry run 当成线上发布。

外部托管先核对远端文件，再发布引用它的页面/清单；官网内置制品随已核对的页面/清单一起部署，随后验证线上字节。使用隔离输出并检查意外残留的下载或更新入口；当前生成器不会自动完成安全撤回。上线、线上核验和撤回是独立状态，恢复旧页面不等于召回已下载的包或回退用户数据库，统一按[发布与恢复流程](../docs/RELEASING.md)执行。

线上 HTTP 报告按时间戳写入已忽略的 `.cloudflare/reports/`。历史发布版本、截图及搜索平台报告留在内部资料，不堆叠到维护入口。HTTP/模拟 UA、真实爬虫、索引、AI 引用及大陆网络访问各自取证，详见 [内容与观测](geo/README.md)。
