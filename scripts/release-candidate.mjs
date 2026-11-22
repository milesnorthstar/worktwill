import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFile, writeFile, mkdir, copyFile, mkdtemp, rm, rename, open } from 'node:fs/promises';
import { resolve, dirname, basename, join } from 'node:path';
import { tmpdir } from 'node:os';
import { fileURLToPath } from 'node:url';
import { platformNames, downloadUrl } from '../website/release.mjs';
import { origin } from '../website/site.config.mjs';

const root = fileURLToPath(new URL('../', import.meta.url));
export const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const json = async path => JSON.parse(await readFile(path, 'utf8'));
const serialize = value => JSON.stringify(value, null, 2) + '\n';
const prepublishChecks = ['installation', 'launch', 'changed-behavior', 'distribution-licenses'];
const requiredChecks = [...prepublishChecks, 'upgrade', 'data-preservation'];
export function run(command, args, cwd = root) {
  const result = spawnSync(command, args, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
  if (result.error || result.status !== 0) throw new Error(`${command} failed: ${result.error?.message ?? result.stderr}`);
  return result.stdout.trim();
}
const git = (...args) => run('git', args);
export function newer(version, previous) {
  // 当前公开通道只接受完整数字版本；预览性质由Release标记表达，不猜测预发行排序。
  assert.match(version, /^\d+\.\d+\.\d+$/);
  if (!previous) return true;
  assert.match(previous, /^\d+\.\d+\.\d+$/);
  const a = version.split('.').map(Number), b = previous.split('.').map(Number);
  for (let i = 0; i < 3; i++) if (a[i] !== b[i]) return a[i] > b[i];
  return false;
}
export function validateReport(manifest, report, phase = 'complete') {
  assert.ok(['prepublish', 'complete'].includes(phase), '无效验收阶段');
  assert.equal(report.candidate, digest(serialize(manifest)), '验收报告不属于这份候选清单');
  assert.equal(report.source, manifest.source.commit);
  assert.equal(report.target, manifest.target);
  assert.equal(report.version, manifest.record.version);
  assert.ok(report.tester?.trim() && report.environment?.trim(), '缺少验收人员或Agent、实际系统环境');
  assert.ok(Number.isFinite(Date.parse(report.completed_at)), '缺少验收时间');
  assert.equal(report.native, true, '必须在对应平台实际安装候选包');
  assert.equal(report.rebuilt, false, '不能用重新构建的包替代云端制品');
  // 正式清单上线前，旧版无法发现新版；升级与数据保持属于发布后的完成条件。
  if (phase === 'prepublish') {
    for (const name of ['upgrade', 'data-preservation']) {
      assert.ok(['not-run', 'blocked', 'passed'].includes(report.checks?.[name]?.status), `${name} 已失败或状态无效，不能发布`);
    }
  }
  if (phase === 'complete') {
    assert.match(report.previous_version ?? '', /^\d+\.\d+\.\d+$/);
    assert.ok(newer(report.version, report.previous_version), '升级验收必须从较低版本开始');
  }
  for (const name of phase === 'prepublish' ? prepublishChecks : requiredChecks) {
    assert.equal(report.checks?.[name]?.status, 'passed', `${name} 尚未通过`);
    assert.ok(report.checks[name].evidence?.trim(), `${name} 缺少脱敏证据引用和观察结果`);
  }
}
async function versions() {
  const version = (await json(join(root, 'package.json'))).version;
  newer(version);
  assert.equal((await json(join(root, 'src-tauri/tauri.conf.json'))).version, version);
  // Cargo负责解析清单及锁文件，避免手写TOML解析器。
  const metadata = JSON.parse(run('cargo', ['metadata', '--locked', '--no-deps', '--format-version', '1']));
  for (const name of ['worktwill-core', 'worktwill-desktop']) {
    assert.equal(metadata.packages.find(p => p.name === name)?.version, version, `${name}版本不一致`);
  }
  return version;
}
export async function buildProvenance() {
  const version = await versions();
  assert.equal(git('status', '--porcelain', '--untracked-files=normal'), '', '构建候选要求干净工作区');
  return {
    commit: git('rev-parse', 'HEAD'), version,
    target: process.platform === 'darwin' ? `darwin-${process.arch === 'arm64' ? 'aarch64' : 'x86_64'}` : 'windows-x86_64',
    config_sha256: digest(serialize(await json(join(root, 'src-tauri/tauri.conf.json')))),
    environment: `${process.platform}/${process.arch}; ${run('rustc', ['--version'])}; ${process.version}`,
    run: process.env.GITHUB_RUN_ID ? `https://github.com/milesnorthstar/worktwill/actions/runs/${process.env.GITHUB_RUN_ID}` : null,
    attempt: process.env.GITHUB_RUN_ATTEMPT ?? null,
  };
}
export function verifySignature(update, signature, version) {
  run('cargo', ['run', '--locked', '-p', 'worktwill-desktop', '--features', 'release-tools', '--bin', 'verify-release', '--', update, signature, join(root, 'src-tauri/tauri.conf.json'), version]);
}
async function artifact(directory, path, version, target) {
  const bytes = await readFile(path);
  const sha256 = digest(bytes);
  const filename = `WorkTwill-${version}-${target}-${sha256.slice(0, 12)}-${basename(path)}`;
  assert.match(filename, /^[\w.-]+$/);
  await copyFile(path, join(directory, filename));
  return { filename, bytes: bytes.length, sha256 };
}
export async function recordCandidate(options) {
  const { target, version, installer, update, provenance, notes, output } = options;
  assert.ok(platformNames[target], '不支持的目标平台');
  newer(version);
  const source = await json(provenance);
  assert.equal(source.target, target);
  assert.equal(source.version, version);
  assert.match(source.commit, /^[a-f\d]{40}$/);
  assert.equal(JSON.parse(git('show', `${source.commit}:package.json`)).version, version);
  assert.equal(digest(serialize(JSON.parse(git('show', `${source.commit}:src-tauri/tauri.conf.json`)))), source.config_sha256, '源码提交与公钥配置不一致');
  // 以构建时的源码配置为依据，拒绝拿另一个分支的公钥登记候选。
  assert.equal(source.config_sha256, digest(serialize(await json(join(root, 'src-tauri/tauri.conf.json')))));
  if (target.startsWith('darwin')) assert.ok(installer.endsWith('.dmg') && update.endsWith('.app.tar.gz'));
  else assert.ok(installer.endsWith('.exe') && resolve(installer) === resolve(update), 'Windows安装与更新必须是同一EXE');
  for (const path of new Set([installer, update, update + '.sig'])) {
    assert.equal(digest(await readFile(path)), source.artifacts?.[basename(path)], '制品不是来源记录中的构建结果');
  }
  verifySignature(update, update + '.sig', version);
  const releaseNotes = await readFile(notes, 'utf8');
  assert.ok(releaseNotes.trim(), '发行说明不能为空');
  const directory = resolve(output);
  await mkdir(directory, { recursive: false });
  const signature = (await readFile(update + '.sig', 'utf8')).trim();
  const record = { version, stage: '预览版', stage_en: 'Preview', notes: releaseNotes,
    ...await artifact(directory, installer, version, target),
    update: { ...await artifact(directory, update, version, target), signature } };
  const registry = await json(join(root, 'website/release.json'));
  const manifest = { schema: 1, target, source, baseline: registry.platforms[target] ?? null, record };
  await writeFile(join(directory, record.update.filename + '.sig'), signature + '\n', { flag: 'wx' });
  await copyFile(join(root, 'docs/INSTALLATION.txt'), join(directory, 'WorkTwill-INSTALL.txt'));
  await writeFile(join(directory, 'candidate.json'), serialize(manifest), { flag: 'wx' });
  const sums = [];
  for (const filename of new Set([record.filename, record.update.filename, record.update.filename + '.sig', 'candidate.json', 'WorkTwill-INSTALL.txt'])) {
    sums.push(`${digest(await readFile(join(directory, filename)))}  ${filename}`);
  }
  await writeFile(join(directory, 'SHA256SUMS.txt'), sums.join('\n') + '\n', { flag: 'wx' });
  await handoff(join(directory, 'candidate.json'), join(directory, 'acceptance.template.json'));
  return { candidate: join(directory, 'candidate.json'), sha256: digest(serialize(manifest)), state: 'candidate-prepared' };
}
export async function verifyCandidate(path) {
  const manifest = await json(path);
  assert.equal(manifest.schema, 1);
  assert.ok(platformNames[manifest.target]);
  assert.match(manifest.source.commit, /^[a-f\d]{40}$/);
  assert.equal(manifest.source.target, manifest.target);
  assert.equal(manifest.source.version, manifest.record.version);
  assert.equal(manifest.source.config_sha256, digest(serialize(await json(join(root, 'src-tauri/tauri.conf.json')))), '请在候选源码配置下验签');
  newer(manifest.record.version);
  if (manifest.target.startsWith('darwin')) {
    assert.ok(manifest.record.filename.endsWith('.dmg') && manifest.record.update.filename.endsWith('.app.tar.gz'));
  } else {
    assert.equal(manifest.record.filename, manifest.record.update.filename);
    assert.ok(manifest.record.filename.endsWith('.exe'));
  }
  const directory = dirname(resolve(path));
  for (const entry of [manifest.record, manifest.record.update]) {
    assert.match(entry.filename, /^[\w.-]+$/);
    const bytes = await readFile(join(directory, entry.filename));
    assert.equal(bytes.length, entry.bytes);
    assert.equal(digest(bytes), entry.sha256, `制品摘要不一致：${entry.filename}`);
    assert.ok(Object.values(manifest.source.artifacts ?? {}).includes(entry.sha256), '候选未绑定构建制品');
  }
  assert.equal((await readFile(join(directory, manifest.record.update.filename + '.sig'), 'utf8')).trim(), manifest.record.update.signature);
  verifySignature(join(directory, manifest.record.update.filename), join(directory, manifest.record.update.filename + '.sig'), manifest.record.version);
  return manifest;
}
export async function handoff(path, output) {
  const manifest = await json(path);
  const report = { candidate: digest(serialize(manifest)), source: manifest.source.commit, target: manifest.target,
    version: manifest.record.version, tester: '', environment: '', previous_version: '', completed_at: '', native: false, rebuilt: false,
    checks: Object.fromEntries(requiredChecks.map(name => [name, { status: 'not-run', evidence: '' }])) };
  await writeFile(output, serialize(report), { flag: 'wx' });
  return { state: 'awaiting-native-acceptance', report: output, candidate: report.candidate,
    instructions: '读取docs/RELEASING.md中的跨机验收；验证候选包，禁止重新构建或使用真实工作数据。' };
}
export async function accept(path, reportPath, phase = 'complete') {
  const manifest = await verifyCandidate(path);
  const report = await json(reportPath);
  validateReport(manifest, report, phase);
  return { state: phase === 'prepublish' ? 'candidate-ready-to-publish' : 'native-upgrade-verified', candidate: report.candidate, report: resolve(reportPath) };
}
export async function promote(path, reportPath) {
  const manifest = await verifyCandidate(path);
  validateReport(manifest, await json(reportPath), 'prepublish');
  const registryPath = join(root, 'website/release.json');
  // 仅序列化本机提升；跨机器发布仍必须走同一PR/发布协调者。
  const lock = await open(registryPath + '.lock', 'wx');
  try {
    const registry = await json(registryPath);
    assert.deepEqual(registry.platforms[manifest.target] ?? null, manifest.baseline, '目标平台记录已变化，拒绝过期提升');
    assert.ok(newer(manifest.record.version, manifest.baseline?.version), '必须使用更高版本');
    const directory = dirname(resolve(path));
    await mkdir(join(root, 'deliverables'), { recursive: true });
    for (const entry of [manifest.record, manifest.record.update]) {
      const destination = join(root, 'deliverables', entry.filename);
      try {
        assert.equal(digest(await readFile(destination)), entry.sha256, '拒绝覆盖不同字节的已有制品');
      } catch (error) {
        if (error.code !== 'ENOENT') throw error;
        await copyFile(join(directory, entry.filename), destination);
      }
    }
    registry.platforms[manifest.target] = { ...manifest.record, published_at: new Date().toISOString(),
      source: manifest.source.commit, candidate: digest(serialize(manifest)) };
    await writeFile(registryPath + '.tmp', serialize(registry), { flag: 'wx' });
    await rename(registryPath + '.tmp', registryPath);
    return { state: 'registry-prepared-not-published', target: manifest.target, version: manifest.record.version };
  } finally {
    await lock.close();
    await rm(registryPath + '.lock');
  }
}
export async function verifyOnline(registry, fetcher = fetch) {
  const results = [];
  async function checked(url, expected) {
    assert.equal(new URL(url).protocol, 'https:');
    const response = await fetcher(url, { signal: AbortSignal.timeout(120000), cache: 'no-store' });
    assert.ok(response.ok, `${url}: HTTP ${response.status}`);
    const bytes = Buffer.from(await response.arrayBuffer());
    if (expected) {
      assert.equal(bytes.length, expected.bytes);
      assert.equal(digest(bytes), expected.sha256, url);
    }
    return bytes;
  }
  for (const [target, entry] of Object.entries(registry.platforms)) {
    await checked(new URL(downloadUrl(entry), origin).href, entry);
    if (entry.update) {
      const url = new URL(downloadUrl(entry.update), origin).href;
      await checked(url, entry.update);
      const manifest = JSON.parse((await checked(`${origin}/updates/${registry.channel}/${target}.json`)).toString());
      assert.equal(manifest.version, entry.version);
      assert.equal(manifest.platforms[target].url, url);
      assert.equal(manifest.platforms[target].signature, entry.update.signature);
    }
    results.push({ target, version: entry.version, sha256: entry.sha256 });
  }
  return { state: 'online-bytes-verified', results, nativeUpgrade: 'separate acceptance required' };
}
export async function archive(path, reportPath) {
  const manifest = await verifyCandidate(path);
  validateReport(manifest, await json(reportPath), 'prepublish');
  const tag = `v${manifest.record.version}`, repository = 'milesnorthstar/worktwill';
  const refs = JSON.parse(run('gh', ['api', `repos/${repository}/git/matching-refs/tags/${tag}`]));
  const ref = refs.find(value => value.ref === `refs/tags/${tag}`);
  if (ref) {
    const sha = ref.object.type === 'tag' ? JSON.parse(run('gh', ['api', `repos/${repository}/git/tags/${ref.object.sha}`])).object.sha : ref.object.sha;
    assert.equal(sha, manifest.source.commit, '已有标签指向不同源码，拒绝复用');
  }
  // 已有Release只允许继续同一源码的草稿；不覆盖制品、不移动已发布标签。
  const releases = JSON.parse(run('gh', ['api', '--paginate', '--slurp', `repos/${repository}/releases`])).flat();
  const existing = releases.find(release => release.tag_name === tag);
  if (existing) {
    assert.equal(existing.draft, true, '已公开Release不可作为候选暂存');
    assert.equal(existing.target_commitish, manifest.source.commit, 'Release源码不一致');
  } else {
    const notesDirectory = await mkdtemp(join(tmpdir(), 'worktwill-release-notes-'));
    try {
      const notes = join(notesDirectory, 'notes.txt');
      await writeFile(notes, manifest.record.notes);
      run('gh', ['release', 'create', tag, '--repo', repository, '--target', manifest.source.commit, '--draft', '--prerelease', '--title', `${tag} Preview`, '--notes-file', notes]);
    } finally { await rm(notesDirectory, { recursive: true, force: true }); }
  }
  const directory = dirname(resolve(path));
  const assets = [manifest.record.filename, manifest.record.update.filename, manifest.record.update.filename + '.sig'];
  // 公共说明使用平台前缀，允许不同平台在同一个版本草稿中独立归档。
  for (const name of ['candidate.json', 'SHA256SUMS.txt', 'WorkTwill-INSTALL.txt']) {
    const renamed = `${manifest.target}-${name}`;
    await copyFile(join(directory, name), join(directory, renamed));
    assets.push(renamed);
  }
  const listing = JSON.parse(run('gh', ['release', 'view', tag, '--repo', repository, '--json', 'assets']));
  const temp = await mkdtemp(join(tmpdir(), 'worktwill-archive-'));
  try {
    for (const name of new Set(assets)) {
      if (!listing.assets.some(asset => asset.name === name)) run('gh', ['release', 'upload', tag, join(directory, name), '--repo', repository]);
      run('gh', ['release', 'download', tag, '--repo', repository, '--pattern', name, '--dir', temp]);
      assert.equal(digest(await readFile(join(temp, name))), digest(await readFile(join(directory, name))), `归档字节不同：${name}`);
    }
  } finally { await rm(temp, { recursive: true, force: true }); }
  return { state: 'draft-archived-not-published', tag, source: manifest.source.commit };
}

export async function main(args) {
  const command = args.shift();
  function option(name) {
    const index = args.indexOf(name);
    assert.ok(index >= 0 && args[index + 1] && !args[index + 1].startsWith('--'), `缺少${name}`);
    return args[index + 1];
  }
  const candidate = () => resolve(option('--candidate'));
  if (command === 'verify') return { state: 'candidate-integrity-verified', manifest: await verifyCandidate(candidate()) };
  if (command === 'handoff') return handoff(candidate(), resolve(option('--output')));
  if (command === 'accept') return accept(candidate(), resolve(option('--report')), args.includes('--phase') ? option('--phase') : 'complete');
  if (command === 'promote') return promote(candidate(), resolve(option('--report')));
  if (command === 'archive') return archive(candidate(), resolve(option('--report')));
  if (command === 'online') return verifyOnline(await json(join(root, 'website/release.json')));
  throw new Error('commands: verify | handoff | accept | archive | promote | online; see docs/RELEASING.md');
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main(process.argv.slice(2)).then(result => console.log(serialize(result))).catch(error => { console.error(error.message); process.exitCode = 1; });
}
