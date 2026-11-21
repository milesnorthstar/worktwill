// 官网与更新清单共同读取同一份逐平台发布记录。
export function currentRelease(registry) {
  const primary = registry.platforms[registry.primary];
  if (!primary) throw new Error('发行记录缺少主平台');
  return { ...primary, platform: registry.primary, platforms: registry.platforms, channel: registry.channel };
}
export const platformNames = {
  'darwin-aarch64': { zh: 'macOS · Apple 芯片', en: 'macOS · Apple Silicon' },
  'darwin-x86_64': { zh: 'macOS · Intel', en: 'macOS · Intel' },
  'windows-x86_64': { zh: 'Windows · x64', en: 'Windows · x64' },
};
export function downloadUrl(release) { return release.url ?? `/downloads/${release.filename}`; }
