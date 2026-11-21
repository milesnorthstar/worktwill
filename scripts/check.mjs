import { spawnSync } from 'node:child_process';
import { existsSync, statSync } from 'node:fs';
import { resolve } from 'node:path';

const option = process.argv.indexOf('--fixtures');
const root = process.env.WORKTWILL_TEST_ROOT || (option >= 0 ? process.argv[option + 1] : undefined);
if (!root || !existsSync(root) || !statSync(root).isDirectory()) {
  console.error('请指定已确认的临时测试目录：pnpm check --fixtures /path/to/fixture-root');
  process.exit(1);
}
const env = { ...process.env, WORKTWILL_TEST_ROOT: resolve(root) };
const steps = [
  ['cargo', ['run', '-p', 'worktwill-core', '--bin', 'export-types', '--', '--check']],
  ['pnpm', ['build']],
  ['pnpm', ['test:ui']],
  ['cargo', ['fmt', '--all', '--', '--check']],
  ['cargo', ['test', '--workspace']],
  ['cargo', ['clippy', '--workspace', '--all-targets', '--', '-D', 'warnings']],
];
for (const [command, args] of steps) {
  console.log(`\n${command} ${args.join(' ')}`);
  // Windows 的 pnpm 是 .cmd；仅对上面固定的检查命令启用命令解释器。
  const result = spawnSync(command, args, {
    env,
    stdio: 'inherit',
    shell: process.platform === 'win32' && command === 'pnpm',
  });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}
