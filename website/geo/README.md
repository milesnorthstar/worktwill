# 官网内容与 SEO/GEO 观测

本文件维护内容与观测方法，历史搜索平台截图和初始报告留在内部资料。`questions.json` 保留固定问题集；它是观测输入，不是已取得的搜索/模型结果。

## 内容约束

产品说明、场景、下载、使用与隐私页均从官网模板/词典生成。公开产品事实须与应用行为、发行记录及已知限制一致。版本/阶段来自 `release.json`；正文核对日期、JSON-LD 和地图保持一致，不按构建日期自动刷新。

英文默认 `/`，中文 `/zh/`，旧 `/en/` 通过 301 转向。HTTP 审计须检查成功响应与目标正文，不能只检查状态码。`llms.txt` 提供公开摘要与导航，不承诺索引、排名或 AI 引用。

## 命令与证据

在根目录执行：

```sh
pnpm website:check
node --test website/scripts/audit-geo.test.mjs
pnpm website:geo:audit
# 以下仅预览 URL 和公开验证键，不主动提交。
pnpm website:indexnow
# 内容实际新增/修改/删除，且任务已授权通知搜索引擎时执行。
pnpm website:indexnow --submit
```

线上 HTTP 审计与 IndexNow 提交的报告写入 `website/.cloudflare/reports/`，按时间戳保存且不覆盖历史。该目录被忽略，不能将报告当成公开构建依赖。IndexNow 的验证字符串按协议公开，不是账户访问凭据；提交只应随内容变化进行，不循环刷量。200/202 是协议接收状态，不等于收录。

## 效果复查

1. Google Search Console 查看地图读取、索引覆盖、展示、点击和实际页面。发现 URL、已索引和排名分别记录。
2. Bing AI Performance 记录统计窗口、引用来源、Grounding Queries 与 URL。只解释其覆盖的产品，不外推到所有 AI。
3. 对固定问题集使用新会话独立抽样，记录平台/模型、日期/地区、搜索状态、问题、原答案、引用和准确性。旧路由先按站点路由映射到当前公开页面，不把自己会话中的项目上下文当成独立发现证据。
4. 品牌提及、官网引用和答案准确性分别记录；未执行或不可读取用 null 和原因，不能填 0 或推断成功。
5. 根据实测查询改进内容，不生成无依据的比较与宣传结论，不为观测自动增加统计脚本、下载归因或第二套任务系统。

模拟 UA 仅证明该次请求可读，不证明真实爬虫访问。大陆网络、真实爬虫日志、搜索索引及模型引用各自验证；无法取得日志不代表零次访问。当前不包含百度的验证/提交流程。

参考：[Google AI features](https://developers.google.com/search/docs/appearance/ai-features)、[OpenAI bots](https://platform.openai.com/docs/bots)、[Bing AI Performance](https://blogs.bing.com/webmaster/2026/2/Introducing-AI-Performance-in-Bing-Webmaster-Tools-Public-Preview/)、[IndexNow](https://www.indexnow.org/documentation)。
