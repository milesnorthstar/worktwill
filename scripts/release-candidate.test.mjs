import assert from 'node:assert/strict';
import test from 'node:test';
import { digest, newer, validateReport, verifyOnline, handoff, run, verifyMacBundle, verifyMacUpdate } from './release-candidate.mjs';
import { mkdtemp, readFile, writeFile, rm, mkdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

test('macOS bundles explicitly request complete ad-hoc signing without a paid identity', async () => {
  const config = JSON.parse(await readFile(new URL('../src-tauri/tauri.conf.json', import.meta.url), 'utf8'));
  assert.equal(config.bundle.macOS.signingIdentity, '-');
});

test('macOS verification rejects executable-only signatures and damaged packaged resources', { skip: process.platform !== 'darwin' }, async () => {
  const directory = await mkdtemp(join(tmpdir(), 'worktwill-codesign-test-'));
  try {
    const app = join(directory, 'WorkTwill.app');
    const executable = join(app, 'Contents/MacOS/fixture');
    const resource = join(app, 'Contents/Resources/fixture.txt');
    const archive = join(directory, 'WorkTwill.app.tar.gz');
    await mkdir(join(app, 'Contents/MacOS'), { recursive: true });
    await mkdir(join(app, 'Contents/Resources'));
    const source = join(directory, 'fixture.c');
    await writeFile(source, 'int main(void) { return 0; }');
    run('cc', [source, '-Wl,-adhoc_codesign', '-o', executable]);
    await writeFile(join(app, 'Contents/Info.plist'), '<?xml version="1.0"?><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>app.worktwill.signature-fixture</string><key>CFBundleExecutable</key><string>fixture</string><key>CFBundlePackageType</key><string>APPL</string></dict></plist>');
    await writeFile(resource, 'original');
    // 重现旧包只签可执行文件、未封印整个应用资源的故障。
    assert.throws(() => verifyMacBundle(app), /code has no resources/);
    run('codesign', ['--force', '--sign', '-', app]);
    verifyMacBundle(app);
    run('tar', ['-czf', archive, '-C', directory, 'WorkTwill.app']);
    await verifyMacUpdate(archive);
    await writeFile(resource, 'changed after signing');
    run('tar', ['-czf', archive, '-C', directory, 'WorkTwill.app']);
    await assert.rejects(verifyMacUpdate(archive), /codesign failed/);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

const manifest = { target: 'windows-x86_64', source: { commit: 'a'.repeat(40) }, record: { version: '1.4.0' } };
function report() {
  return { candidate: digest(JSON.stringify(manifest, null, 2) + '\n'), source: manifest.source.commit,
    target: manifest.target, version: '1.4.0', previous_version: '1.3.1', tester: 'Windows Agent',
    environment: 'Windows 11 x64, isolated test account', completed_at: new Date().toISOString(), native: true, rebuilt: false,
    checks: Object.fromEntries(['installation', 'launch', 'upgrade', 'data-preservation', 'changed-behavior', 'distribution-licenses']
      .map(name => [name, { status: 'passed', evidence: 'sanitized evidence reference and observed result' }])) };
}
test('acceptance binds exact candidate, source, platform and real native checks', () => {
  validateReport(manifest, report());
  for (const change of [r => r.candidate = 'wrong', r => r.source = 'b'.repeat(40), r => r.target = 'darwin-aarch64',
    r => r.native = false, r => r.rebuilt = true, r => r.environment = '', r => r.previous_version = '1.4.0',
    r => r.checks.upgrade.status = 'blocked', r => r.checks.installation.evidence = '', r => delete r.checks['distribution-licenses']]) {
    const value = report(); change(value); assert.throws(() => validateReport(manifest, value));
  }
});
test('publication preparation does not depend on an unpublished update feed', () => {
  const value = report();
  value.previous_version = '';
  for (const name of ['upgrade', 'data-preservation']) {
    value.checks[name] = { status: 'not-run', evidence: '' };
  }
  validateReport(manifest, value, 'prepublish');
  assert.throws(() => validateReport(manifest, value, 'complete'));
  value.checks.upgrade.status = 'blocked';
  validateReport(manifest, value, 'prepublish');
  value.checks.upgrade.status = 'failed';
  assert.throws(() => validateReport(manifest, value, 'prepublish'));
});
test('publication still rejects missing native installation, wrong bytes and missing licenses', () => {
  for (const change of [r => r.native = false, r => r.candidate = 'different',
    r => r.checks.installation.status = 'not-run', r => r.checks['distribution-licenses'].status = 'blocked']) {
    const value = report(); change(value);
    assert.throws(() => validateReport(manifest, value, 'prepublish'));
  }
  assert.throws(() => validateReport(manifest, report(), 'unknown'));
});
test('handoff is explicitly unpassed, source-bound and refuses to overwrite evidence', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'worktwill-release-test-'));
  try {
    const candidate = join(directory, 'candidate.json'), output = join(directory, 'report.json');
    await writeFile(candidate, JSON.stringify(manifest));
    assert.equal((await handoff(candidate, output)).state, 'awaiting-native-acceptance');
    const value = JSON.parse(await readFile(output, 'utf8'));
    assert.equal(value.native, false);
    assert.equal(value.checks.upgrade.status, 'not-run');
    assert.throws(() => validateReport(manifest, value));
    await assert.rejects(handoff(candidate, output), /EEXIST/);
  } finally { await rm(directory, { recursive: true, force: true }); }
});
test('only increasing numeric versions can be promoted', () => {
  assert.equal(newer('1.3.2', '1.3.1'), true);
  assert.equal(newer('1.4.0', '1.3.9'), true);
  assert.equal(newer('1.3.1', '1.3.1'), false);
  assert.equal(newer('1.2.9', '1.3.1'), false);
  assert.throws(() => newer('1.3.2-preview', '1.3.1'));
});
test('online verification covers every platform installer, update and manifest', async () => {
  const data = Buffer.from('fixture');
  const entry = { version: '1.4.0', filename: 'fixture.exe', bytes: data.length, sha256: digest(data) };
  entry.update = { ...entry, signature: 'signature' };
  const registry = { channel: 'stable', platforms: { 'windows-x86_64': entry, 'darwin-aarch64': entry } };
  const requests = [];
  const fetcher = async url => {
    requests.push(url);
    const target = url.includes('darwin') ? 'darwin-aarch64' : 'windows-x86_64';
    return new Response(url.endsWith('.json') ? JSON.stringify({ version: '1.4.0', platforms: {
      [target]: { url: 'https://worktwill.haoclub.com/downloads/fixture.exe', signature: 'signature' },
    } }) : data);
  };
  assert.equal((await verifyOnline(registry, fetcher)).results.length, 2);
  assert.equal(requests.length, 6);
  await assert.rejects(verifyOnline(registry, async () => new Response('wrong bytes')));
  await assert.rejects(verifyOnline(registry, async () => new Response('', { status: 404 })));
});
