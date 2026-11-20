# 官网本地验收记录

日期：2026-11-20。范围为中英文静态官网及当前026候选包分发，不涉及桌面业务实现验收。

## GEO首轮增量（当前线上版本）

当前版本 **`a76059cf-1d85-4729-a3c8-1011a89e1118`**。按用户同意的方案新增产品事实、客户合同场景、下载与支持三个页面及英文对应，共12个可索引URL；首页和其他文档增加静态内容导航。用户已决定完整项目开源，同步PRD、README和双语官网，不擅自选许可证或声称仓库已发布。

- `pnpm website:check` 通过12页的翻译、SEO、hreflang互链、内容导航、发行数据与校验值检查。
- 浏览器核对事实页、说明页语言互切、英文三类新页面320像素布局，无横向溢出。
- `pnpm website:geo:audit` 完整线上HTTP检查通过；安装包大小和SHA-256一致，robots/llms/地图/图片与双语404符合预期。模拟OAI-SearchBot、Googlebot和bingbot的请求均返回200，仅作为UA模拟，不充当真实爬虫记录。
- Google Search Console已通过现有主域名所有权自动验证官网资源；地图成功提交后，初始短暂“无法抓取”随后更新为“已成功处理站点地图”，发现12个页面。最终状态证据：[Google地图](../docs/validation/website/geo-google-processed.jpg)。不把发现数当索引数。
- Bing以公开HTML标记验证站点，地图已提交并显示Processing：[Bing地图](../docs/validation/website/geo-bing-submitted.jpg)。AI Performance可用，当前窗口2026-08-20至2026-11-19显示0次引用，窗口截止本次发布前，不能用于衡量本轮结果：[引用基线](../docs/validation/website/geo-bing-ai-baseline.jpg)。
- IndexNow提交12个URL得到202，校验待处理，不等于收录成功。完整HTTP、配置检查及IndexNow响应分别保存在 `docs/validation/website/geo-http-2026-11-20.json`、`geo-cloudflare-2026-11-20.json`、`indexnow-2026-11-20.json`。
- Cloudflare仅只读核对，无安全设置变更；真实AI爬虫明细日志尚未取得。百度按用户明确指示从本轮范围移除；逐问跨平台模型样本和实际AI来路下载归因尚待后续观测。已建立12个固定中英文问题及初始基线，未观测值为null。

方法、证据边界与后续工作见[geo/README.md](geo/README.md)。下方旧版本条目保留历史含义。

## 中英文与SEO增量（历史发布记录）

用户要求官网支持中文与英文，并核查SEO。保留既有Vite静态多页方案，新增构建期模板和翻译词典，共用页面结构；不改变桌面端架构。当时部署版本 **`bd23e7cb-7bdb-495d-be76-3562db99d222`**；当前状态以上方GEO首轮增量为准。

- 中文首页 `/`、使用说明 `/guide`、隐私说明 `/privacy`；英文对应 `/en/`、`/en/guide`、`/en/privacy`。双语404均返回相应语言的HTML及404状态，不进入站点地图。
- 顶部语言切换使用普通链接，在说明页切换仍保留页面对应关系。英文首页、FAQ、安装/隐私说明、演示导航、数据、搜索结果和空状态均完整翻译。
- 6个页面在原始HTML提供独立title/description、self canonical、双语和x-default hreflang及回指关系、JSON-LD、Open Graph/Twitter元数据与1200×630PNG分享图；站点地图含6个URL及每项alternate。禁用JS仍可读正文、查看首屏示例和下载安装包。
- `pnpm website:check` 构建及自动验收通过，发布前自动执行。缺少译文直接失败；自动检查覆盖站内链接、语言对应关系、原始HTML元数据、分享图尺寸、结构化数据及不虚构评分/价格。
- Chrome浏览器：英文搜索onboarding返回两条入职示例、无结果提示英文、使用说明中英互切正确。英文390/320、中英文320像素及英文说明/隐私页320像素无横向溢出。没有控制台warn/error。截图见[英文线上首页](../docs/validation/website/live-en-home.jpg)。
- 实际curl取得六个页面200，检查原始HTTP HTML中的语言、canonical、alternate和JSON-LD；中英文404为404、双语分享图和站点地图为200；重新下载候选包并核对大小及SHA-256。见[双语SEO线上证据](../docs/validation/website/live-i18n-seo-2026-11-20.json)。
- 此次历史发布时未完成站长平台验证与提交。Google、Bing现已验证并提交，百度已按用户指示排除，详见上方当前记录；搜索引擎实际收录/排名及大陆各运营商访问仍未实测。

已执行并通过：

- `pnpm website:build`：首页、安装说明、隐私说明、404及下载制品输出至独立的 `website/dist/`；主JS 13.03 KB（gzip 5.40 KB），无外部字体/图标/统计请求。
- `pnpm website:deploy:check`：Cloudflare官方CLI 1.0.0-rc.2完成静态资源构建及部署dry run；加入正式域名和站点地图后读取13个静态文件，无业务binding。
- `pnpm build`：原有桌面端TypeScript/Vite构建通过。未因官网修改Rust或DTO，不运行无关原生验收。
- 浏览器交互：搜索「入职」「企划」、无结果、Escape清空、工作区切换、网页类型筛选、收藏、动态、FAQ展开均核对实际可见结果。
- 响应式：开发页面1440/390/320像素检查，390和320像素均无横向溢出；使用说明和隐私说明在320像素也无横向溢出。生产预览在320像素检查，同样无溢出；51个图标完成转换，未见未加载图片或控制台错误。
- 构建后逐个解析4个HTML：站内页面、静态资源和锚点均存在；未发现外部资源依赖。
- 通过生产预览HTTP实际下载ZIP；构建目录ZIP及HTTP下载ZIP的文件大小、SHA-256、ZIP CRC均核对通过：6,613,848字节，`74a31446435ecc60ec899f2d42f43e647a02d6596dd6d3a32bf8b9a2f6880e43`。

浏览器截图：[首页窄屏](../docs/validation/website/home.jpg)、[首页完整窄屏](../docs/validation/website/home-full.jpg)。浅色文字在最终构建中已加深；手机图标导航保留可访问名称。额外原始CDP视口调整被浏览器审批拒绝，未执行，也未尝试绕过；不把该检查写成通过。

## 首次线上发布与验证（历史记录）

用户完成Cloudflare官方登录，并明确授权在现有唯一主域名下添加自定义子域名。通过CLI核实账户、唯一active zone `haoclub.com`、目标DNS记录不存在且 `worktwill-website` Worker未占用后，执行 `pnpm website:deploy`。

- 官网：**https://worktwill.haoclub.com/**。
- Worker：`worktwill-website`，当时部署版本 `911853be-0410-4155-a70e-fc7a24547843`；首次发布版本为 `cb817583-c49b-404f-be42-3790e1551f58`，随后补齐静态404页路由配置。
- Cloudflare自动创建代理DNS记录并完成HTTPS访问；该记录标记为Worker管理。没有修改主域名及其他站点的DNS或路由。
- 线上首页、安装说明、隐私说明、站点地图、robots和favicon均返回200；不存在页面返回404及已设计的HTML错误页。`.html`说明链接自动跳转到对应无扩展名路径。
- 实际下载官网ZIP，并核对大小、SHA-256、ZIP CRC及 `Content-Disposition: attachment`。
- CSP、禁用嵌入、内容类型保护等静态响应头已在线上生效。
- 实际Chrome打开正式域名，搜索「入职」返回两条对应示例资料；51个图标加载完成，未见控制台警告或错误，1808像素视口无横向溢出。线上截图见[线上首页](../docs/validation/website/live-home.jpg)。
- curl及浏览器的具体结果见[线上HTTP证据](../docs/validation/website/live-http-2026-11-20.json)。Python urllib默认客户端收到Cloudflare 1010拒绝，未将该客户端检查计为通过，未修改zone安全设置；正常curl及Chrome客户端访问成功。

未执行：

- 从中国大陆网络访问与下载；当前网络验证不表示大陆各运营商可用。
- 当前包本身的原生界面、各Office保存行为、Windows、Apple签名、公证与正式发行验收。

官网中的产品能力与限制以当前PRD、README、USER_GUIDE为依据；WorkTwill仍沿用暂用品牌，不新增中文品牌、收费承诺或已开源声明。
