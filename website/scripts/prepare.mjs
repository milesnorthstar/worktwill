import { createHash } from 'node:crypto';
import { readFile, mkdir, copyFile } from 'node:fs/promises';

const site = new URL('../', import.meta.url);
const release = JSON.parse(await readFile(new URL('release.json', site), 'utf8'));
const source = new URL(`../deliverables/${release.filename}`, site);
const bytes = await readFile(source);
// 发布前核对已验收制品，避免官网误发其他候选包。
if (bytes.length !== release.bytes || createHash('sha256').update(bytes).digest('hex') !== release.sha256) {
  throw new Error('候选包与 release.json 不一致，请核对发行记录。');
}
await mkdir(new URL('public/downloads/', site), { recursive: true });
await copyFile(source, new URL(`public/downloads/${release.filename}`, site));
console.log(`已校验并准备官网直接下载：${release.filename}`);
