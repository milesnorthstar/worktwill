import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { origin, pages, locales, localizedRoute } from '../site.config.mjs';

const exec = promisify(execFile);
const site = new URL('../', import.meta.url);
const { key, endpoint } = JSON.parse(await readFile(new URL('indexnow.json', site), 'utf8'));
const body = {
  host: new URL(origin).hostname,
  key,
  keyLocation: `${origin}/${key}.txt`,
  urlList: Object.values(pages).filter((page) => page.route !== '/404').flatMap((page) => locales.map((locale) => origin + localizedRoute(locale, page.route))),
};
if (!process.argv.includes('--submit')) {
  console.log(JSON.stringify({ endpoint, ...body }, null, 2));
} else {
  // 先确认线上校验文件已发布，再提交公开URL；不携带账户密钥或用户资料。
  const probe = await exec('curl', ['-fsSL', '--max-time', '30', body.keyLocation]);
  if (probe.stdout.trim() !== key) throw new Error('线上IndexNow校验文件不一致，请先部署');
  const reportDirectory = new URL('../.cloudflare/reports/', import.meta.url);
  await mkdir(reportDirectory, { recursive: true });
  const payload = new URL('../.cloudflare/indexnow-body.json', import.meta.url);
  await writeFile(payload, JSON.stringify(body));
  const { stdout } = await exec('curl', ['-sS', '--max-time', '40', '-X', 'POST', '-H', 'Content-Type: application/json', '--data-binary', `@${payload.pathname}`, '-w', '\n%{http_code}', endpoint]);
  const status = Number(stdout.trim().split('\n').at(-1));
  const responseBody = stdout.trim().split('\n').slice(0, -1).join('\n');
  const evidence = { at: new Date().toISOString(), endpoint, status, responseBody, urls: body.urlList, meaning: status === 200 ? 'URL batch accepted, indexing not guaranteed' : status === 202 ? 'URL batch received; key validation pending; indexing not guaranteed' : 'submission failed' };
  // 原始观测留在已忽略目录，按时间保存，避免覆盖历史或混入公开文档。
  await writeFile(new URL(`indexnow-${evidence.at.replaceAll(':', '-')}.json`, reportDirectory), JSON.stringify(evidence, null, 2) + '\n', { flag: 'wx' });
  console.log(JSON.stringify(evidence, null, 2));
  if (![200, 202].includes(status)) process.exitCode = 1;
}
