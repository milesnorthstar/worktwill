import { createHash } from 'node:crypto';
import { readFile, writeFile, mkdir, copyFile } from 'node:fs/promises';
import {currentRelease,downloadUrl,platformNames} from '../release.mjs';
import {origin} from '../site.config.mjs';

const site = new URL('../', import.meta.url);
const release = currentRelease(JSON.parse(await readFile(new URL('release.json', site), 'utf8')));
await mkdir(new URL('public/downloads/', site), {recursive:true});
await copyFile(new URL('../docs/INSTALLATION.txt', site),new URL('public/downloads/WorkTwill-INSTALL.txt', site));
await mkdir(new URL(`public/updates/${release.channel}/`, site), {recursive:true});
for (const [target, entry] of Object.entries(release.platforms)) {
  if (!platformNames[target] || !/^\d+\.\d+\.\d+(?:-[\w.-]+)?$/.test(entry.version)) throw new Error(`无效发行平台或版本：${target}`);
  for (const artifact of [entry, entry.update].filter(Boolean)) {
    if (!/^[\w.-]+$/.test(artifact.filename) || !/^[a-f\d]{64}$/.test(artifact.sha256)) throw new Error('制品文件名或摘要无效');
    const source = new URL(`../deliverables/${artifact.filename}`, site);
    const bytes = await readFile(source);
    if (bytes.length !== artifact.bytes || createHash('sha256').update(bytes).digest('hex') !== artifact.sha256) throw new Error(`制品与发行记录不一致：${artifact.filename}`);
    if (artifact.url) {
      if (new URL(artifact.url).protocol !== 'https:') throw new Error('正式制品地址必须为HTTPS');
    } else {
      if (bytes.length > 25 * 1024 * 1024) throw new Error('超过静态资源单文件限制，请配置R2制品地址');
      await mkdir(new URL('public/downloads/', site), {recursive:true});
      await copyFile(source, new URL(`public/downloads/${artifact.filename}`, site));
    }
  }
  if (entry.update) {
    const manifest = {version:entry.version,notes:entry.notes??'',pub_date:entry.published_at,
      platforms:{[target]:{url:new URL(downloadUrl(entry.update),origin).href,signature:entry.update.signature}},mirrors:entry.update.mirrors??[]};
    if (!manifest.platforms[target].signature) throw new Error('更新包必须有签名');
    await writeFile(new URL(`public/updates/${release.channel}/${target}.json`,site),JSON.stringify(manifest,null,2)+'\n');
  }
  console.log(`已校验官网制品：${target} ${entry.version}`);
}
