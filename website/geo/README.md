# GEO 内容与观测

2026-11-22 已部署优化（代码 `9c909bd`，Cloudflare版本 `d95a6d16-6cf5-4d94-bfac-24c06be8b72d`）：英文默认 `/`，中文 `/zh/`；产品说明去除旧候选026与开源计划文案，版本/阶段读取发行记录；软件结构化数据关联 GitHub。实质更新页面的日期同步正文、JSON-LD及地图，不按构建日自动刷新。

HTTP 审计同时接受首页页脚与内页内容导航；模拟爬虫必须返回200和目标产品正文，否则失败。临时响应无论成功失败都会清理，成功报告按时间戳独立保存至 `website/.cloudflare/reports/`，不覆盖历史。正式回归：`node --test website/scripts/audit-geo.test.mjs`，隔离HTTP与写入，不代表真实爬虫或线上验收。

本轮HTTP检查通过12页、双语404、抓取辅助文件、分享图、安装包完整性和模拟UA目标正文；旧 `/en/` 六条301单独核对通过。未提交IndexNow，未取得真实爬虫日志或新的搜索/AI引用报表。新报告位于 `website/.cloudflare/reports/geo-http-2026-11-22T00-06-42.261Z.json`；下方站长平台数据均为历史基线，不代表本轮效果。

历史首轮已部署版本：`a76059cf-1d85-4729-a3c8-1011a89e1118`，日期 2026-11-20。

## 已交付

- 中英文产品事实与完整开源计划 `/about`、客户合同场景 `/contract-workspace`、下载与支持范围 `/download`，英文对应 `/en/` 路径。复用模板与翻译生成器，不新增框架或客户端整页翻译。
- 原有首页、说明页均添加静态内容导航，12个可索引URL有canonical、hreflang、站点地图及结构化数据。新增内容有实际核对日期，不能机械每日刷新lastmod。
- `llms.txt` 只提供产品摘要和官网导航，未将其作为引用率或排名保证。
- Google已自动验证官网URL前缀资源，站点地图已成功处理，发现12个页面。站点地图发现数不等于索引数，当前索引、展示与点击待处理。
- Bing已通过首页 `msvalidate.01` 标记验证。地图已提交，状态Processing。AI Performance报表可用。
- IndexNow已提交12个公开URL，返回202，含义为已收到且校验待处理；不等于索引完成。
- Cloudflare只读检查：AI机器人一键保护未启用，没有自定义User-Agent封禁或IP访问规则；Browser Integrity Check仍为on。没有变更安全设置。Google地图实际读取成功；UA模拟不是其他真实爬虫来访证据。

## 固定问题与基线

`questions.json` 固定12个中英文问题，覆盖品牌理解、无品牌场景发现、离线/隐私、平台、开源和保存追踪边界。`baseline-2026-11-20.json` 保存初始状态。

Bing AI Performance当前显示2026-08-20至2026-11-19的Total Citations=0、Avg. Cited Pages=0。统计窗口截止本次更新前，不能称为本轮GEO效果。Google索引量、AI来路访问、AI带来的下载以及逐问模型样本暂未观测，使用null，不填0。

建议在站长报告开始产生新数据后按周复查：

1. Google查看地图读取、索引覆盖、展示/点击和实际命中的页面；不反复重复提交催促。
2. Bing查看AI Performance的引用来源、统计窗口、Grounding Queries与被引用URL，记录原始截图或导出，并与相同窗口比较。报表只覆盖Microsoft Copilots与参与的合作方，不外推到所有AI。
3. 抽样使用同一组问题，在ChatGPT Search、Gemini以及可用的国内平台分别测试；使用新会话，记录平台、模型（若可见）、日期、地区、是否开启搜索、原问题、原答案、引用URL和准确性。品牌问题与无品牌发现问题分别比较，不给答案暗示，不把自己的对话上下文当独立引用。
4. 每条模型结果中 `brandMention`、`officialCitation`、`answerAccuracy` 三项独立记录；只提名字不等于引用，引用官网也不等于描述准确。无法访问或未执行写null并说明原因。
5. 根据实际查询和引用补充缺失内容；实测、编辑器保存或平台支持改变后先更新支持矩阵。新增比较内容前核查竞品，不批量生成无证据的“最佳工具”文章。

没有创建定时任务或第二套任务系统，也没有新增广告、浏览器统计脚本或AI来路追踪服务。未来若需要下载归因，应先确定能从Cloudflare现有聚合数据读取的字段和隐私说明，再决定是否增加采集；当前不宣称已实现转化归因。

## 复查与内容发布命令

```sh
pnpm website:check
pnpm website:deploy
pnpm website:geo:audit

# 先预览URL和公开验证键；上线校验文件后再提交
pnpm website:indexnow
pnpm website:indexnow --submit
```

IndexNow不是每次构建都会调用。只有新增、修改或移除内容时才通知参与的搜索引擎，不重复刷量。`indexnow.json` 的键是协议要求公开的站点验证字符串，不是账户访问密钥；生成文件与线上键一致后才提交。

## 当前未完成与限制

- 用户明确要求忽略百度，不纳入本轮登录、验证、提交或后续待办。
- Cloudflare AI Crawl Control仪表盘导航超时，SQL数据集探查也未成功，没有取得按已验证爬虫身份统计的来访日志。已完成的配置检查、Google地图抓取与UA模拟须分开看待，不把日志缺失解释为零次抓取。
- ChatGPT/Gemini/国内平台的逐问回答样本尚未执行。固定问题和基线已经准备，不虚构初始引用。
- 大陆各运营商的实际访问与下载尚未独立测试。
- 用户已确认公开仓库 https://github.com/milesnorthstar/worktwill；本地官网已接入，许可证与贡献说明以仓库为准。2026-11-22线上HTTP已确认仓库入口与结构化关联；许可证内容仍以仓库为准。

## 依据

- [Google AI features and your website](https://developers.google.com/search/docs/appearance/ai-features)：无需特殊AI文件；先满足搜索技术和内容要求。
- [OpenAI bots](https://platform.openai.com/docs/bots)：搜索抓取与训练用途独立设置。
- [Bing AI Performance](https://blogs.bing.com/webmaster/2026/2/Introducing-AI-Performance-in-Bing-Webmaster-Tools-Public-Preview/)：引用指标、适用来源与限制。
- [IndexNow](https://www.indexnow.org/documentation)：公开键校验与提交响应含义。

线上证据保存在 `docs/validation/website/geo-*.json`、`geo-*.jpg` 和 `indexnow-2026-11-20.json`。相关截图只作本地验收证据，不随官网静态目录发布。
