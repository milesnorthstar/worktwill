import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import { createHash } from 'node:crypto';
import { JSDOM } from 'jsdom';
import { origin, pages, locales, localizedRoute } from '../site.config.mjs';

// 执行真实审计流程，隔离 HTTP 和文件写入；覆盖首页导航与挑战页，不能替代线上验收。
const source = await readFile(new URL('audit-geo.mjs', import.meta.url), 'utf8');
const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;

async function runAudit(failure, script = source) {
  // Windows 检出可能使用 CRLF，移除 import 时需同时消费行尾回车。
  const isolated = script.replace(/^import .*;\r?\n/gm, '')
    .replace("const {currentRelease,downloadUrl}=await import('../release.mjs');", '')
    .replaceAll('import.meta.url', JSON.stringify(new URL('audit-geo.mjs', import.meta.url).href));
  const files = new Map();
  const reports = [];
  let cleaned = false;
  const download = Buffer.from('test release');
  const release = { bytes: download.length, sha256: createHash('sha256').update(download).digest('hex') };
  const exec = async (_command, args) => {
    const url = new URL(args.at(-1));
    const path = url.pathname;
    let body;
    let status = 200;
    let contentType = 'text/html';
    if (path === '/test-download') body = download;
    else if (path.includes('no-such')) { status = 404; body = Buffer.from('not found'); }
    else if (path === '/sitemap.xml') body = Buffer.from(`<urlset>${Array.from({ length: 12 }, () => '<url><loc>test</loc></url>').join('')}</urlset>`);
    else if (/\.(txt|png)$/.test(path)) body = Buffer.from('asset');
    else {
      const lang = path.startsWith('/zh/') ? 'zh-CN' : 'en';
      const blocked = args.includes('-A') && failure;
      if (blocked === 'status') status = 403;
      body = Buffer.from(blocked ? '<html><title>Just a moment</title></html>' : `<html lang="${lang}"><head><link rel="canonical" href="${origin}${path}">${'<link rel="alternate">'.repeat(3)}<script type="application/ld+json">{}</script></head><body><main id="facts">Product facts</main><footer class="footer-links"><a href="/guide">Guide</a></footer></body></html>`);
    }
    files.set(args[args.indexOf('-o') + 1], body);
    files.set(args[args.indexOf('-D') + 1], 'HTTP response');
    return { stdout: JSON.stringify({ http_code: status, url_effective: url.href, content_type: contentType }) };
  };
  const dependencies = {
    assert, readFile: async (path) => path instanceof URL ? JSON.stringify(release) : files.get(path),
    writeFile: async (path, body, options) => reports.push({ path: path.pathname, body: JSON.parse(body), options }),
    mkdtemp: async () => '/tmp/isolated-geo-test', mkdir: async () => {},
    rm: async () => { cleaned = true; }, tmpdir: () => '/tmp', join: (...parts) => parts.join('/'),
    execFile: exec, promisify: (value) => value, createHash, JSDOM, origin, pages, locales, localizedRoute,
    currentRelease: (value) => value, downloadUrl: () => '/test-download', console: { log() {} },
  };
  try {
    await new AsyncFunction(...Object.keys(dependencies), isolated)(...Object.values(dependencies));
    return reports;
  } finally {
    assert.ok(cleaned, '失败与成功都必须清理临时响应');
    if (failure) assert.equal(reports.length, 0, '抓取失败不能生成成功报告');
  }
}

for (const newline of ['\n', '\r\n']) {
  test(`${JSON.stringify(newline)} 源码：首页仅有页脚导航也能完成审计，报告不覆盖历史`, async () => {
    const reports = await runAudit(undefined, source.replace(/\r?\n/g, newline));
    assert.equal(reports.length, 1);
    assert.equal(reports[0].body.pages.length, 12);
    assert.equal(reports[0].body.syntheticAgentProbes.length, 3);
    assert.match(reports[0].path, /\.cloudflare\/reports\/geo-http-.*\.json$/);
    assert.equal(reports[0].options.flag, 'wx');
  });
}
for (const failure of ['status', 'challenge']) {
  test(`模拟爬虫返回 ${failure} 时失败且不写成功报告`, async () => {
    await assert.rejects(runAudit(failure), /无法抓取|非目标页面/);
  });
}
