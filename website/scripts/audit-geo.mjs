import assert from 'node:assert/strict';
import { readFile, writeFile, mkdtemp, mkdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { createHash } from 'node:crypto';
import { JSDOM } from 'jsdom';
import { origin, pages, locales, localizedRoute } from '../site.config.mjs';

const exec = promisify(execFile);
const temp = await mkdtemp(join(tmpdir(), 'worktwill-geo-audit-'));
const routes = Object.values(pages).filter((page) => page.route !== '/404').flatMap((page) => locales.map((locale) => localizedRoute(locale, page.route)));
let sequence = 0;
async function request(path, agent) {
  const file = join(temp, String(sequence++));
  const args = ['-sS', '-L', '--max-time', '40', '-D', `${file}.headers`, '-o', `${file}.body`, '-w', '%{json}'];
  if (agent) args.push('-A', agent);
  const { stdout } = await exec('curl', [...args, new URL(path,origin).href]);
  const transport = JSON.parse(stdout);
  return { path, status: transport.http_code, url: transport.url_effective, body: await readFile(`${file}.body`), headers: await readFile(`${file}.headers`, 'utf8'), contentType: transport.content_type };
}
// 无论成功或失败都清理本次下载的响应，不覆盖历史验收报告。
try {
const pageResults = [];
// 限制同时请求数量；逐页核对实际生产HTML，不把UA模拟当成真实爬虫证据。
for (let offset = 0; offset < routes.length; offset += 4) {
  const batch = await Promise.allSettled(routes.slice(offset, offset + 4).map(async (path) => {
    const response = await request(path);
    assert.equal(response.status, 200, path);
    const document = new JSDOM(response.body.toString()).window.document;
    assert.equal(document.querySelector('link[rel="canonical"]').href, origin + path);
    assert.equal(document.querySelectorAll('link[rel="alternate"]').length, 3);
    assert.equal(document.documentElement.lang, path.startsWith('/zh/') ? 'zh-CN' : 'en');
    assert.ok(document.querySelector('script[type="application/ld+json"]'));
    assert.ok(document.querySelector('.resource-library a[href], .footer-links a[href]'), `${path} 缺少静态内容导航`);
    return { path, status: response.status, bytes: response.body.length, language: document.documentElement.lang, canonical: origin + path };
  }));
  for (const item of batch) {
    if (item.status === 'rejected') throw item.reason;
    pageResults.push(item.value);
  }
}
const probes = [];
for (const path of ['/robots.txt', '/llms.txt', '/sitemap.xml', '/og/en.png', '/og/zh.png', '/no-such-geo-page', '/zh/no-such-geo-page']) {
  const result = await request(path);
  assert.equal(result.status, path.includes('no-such') ? 404 : 200, path);
  if (path === '/sitemap.xml') {
    const document = new JSDOM(result.body.toString(), { contentType: 'application/xml' }).window.document;
    assert.equal(document.querySelectorAll('url > loc').length, routes.length);
  }
  probes.push({ path, status: result.status, bytes: result.body.length });
}
const syntheticAgentProbes = [];
for (const [name, agent] of [
  ['OAI-SearchBot', 'Mozilla/5.0 (compatible; OAI-SearchBot/1.4; +https://openai.com/searchbot)'],
  ['Googlebot', 'Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)'],
  ['bingbot', 'Mozilla/5.0 (compatible; bingbot/2.0; +http://www.bing.com/bingbot.htm)'],
]) {
  const response = await request('/about', agent);
  // HTTP 200 也可能是挑战页，必须确认返回的是目标正文，而非仅记录状态码。
  assert.equal(response.status, 200, `${name} 无法抓取产品页`);
  assert.match(response.contentType, /text\/html/i, name);
  const document = new JSDOM(response.body.toString()).window.document;
  assert.equal(document.querySelector('link[rel="canonical"]')?.href, origin + '/about', `${name} 返回了非目标页面`);
  assert.ok(document.querySelector('#facts'), `${name} 缺少产品事实正文`);
  syntheticAgentProbes.push({ name, status: response.status, meaning: 'Synthetic user-agent probe from current network; not a verified crawler visit' });
}
const {currentRelease,downloadUrl}=await import('../release.mjs');
const release = currentRelease(JSON.parse(await readFile(new URL('../release.json', import.meta.url), 'utf8')));
const download = await request(downloadUrl(release));
assert.equal(download.status, 200);
assert.equal(download.body.length, release.bytes);
assert.equal(createHash('sha256').update(download.body).digest('hex'), release.sha256);
const evidence = { at: new Date().toISOString(), origin, pages: pageResults, probes, syntheticAgentProbes, download: { bytes: download.body.length, sha256: release.sha256 }, verifiedCrawlerVisits: 'unknown; inspect Cloudflare AI Crawl Control for verified identities', mainlandNetworkVerified: false };
const reportDirectory = new URL('../.cloudflare/reports/', import.meta.url);
await mkdir(reportDirectory, { recursive: true });
const report = new URL(`geo-http-${evidence.at.replaceAll(':', '-')}.json`, reportDirectory);
await writeFile(report, JSON.stringify(evidence, null, 2) + '\n', { flag: 'wx' });
console.log(JSON.stringify({ report: report.pathname, ...evidence }, null, 2));
} finally {
  await rm(temp, { recursive: true, force: true });
}
