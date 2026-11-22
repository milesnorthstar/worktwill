import {spawnSync} from 'node:child_process';
import {readFile,writeFile,mkdir,chmod,copyFile,access,readdir} from 'node:fs/promises';
import {resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {digest,buildProvenance,recordCandidate} from './release-candidate.mjs';

const root=fileURLToPath(new URL('../',import.meta.url));
const key=resolve(root,'.release-keys/updater.key');
const configPath=resolve(root,'src-tauri/tauri.conf.json');
const args=process.argv.slice(2);
const command=args.shift();
function option(name,required=true){const i=args.indexOf(name);const value=i<0?undefined:args[i+1];if(required&&!value)throw new Error(`缺少 ${name}`);return value;}
const json=async path=>JSON.parse(await readFile(path,'utf8'));
const exists=async path=>access(path).then(()=>true,()=>false);
function run(parts,env=process.env){
  const result=spawnSync(process.execPath,[resolve(root,'node_modules/@tauri-apps/cli/tauri.js'),...parts.slice(1)],{cwd:root,env,encoding:'utf8',stdio:command==='keygen'?'pipe':'inherit'});
  // signer输出可能包含私钥，不回显生成命令的stdout/stderr。
  if(result.error||result.status!==0)throw new Error(`命令失败：pnpm ${parts[0]} ${parts[1]??''}`);
}

if(command==='keygen'){
  if(await exists(key)||await exists(key+'.pub'))throw new Error('签名密钥已存在，拒绝覆盖或轮换');
  await mkdir(resolve(root,'.release-keys'),{recursive:true,mode:0o700});
  run(['tauri','signer','generate','--ci','--password','','--write-keys',key]);
  await chmod(key,0o600);
  const config=await json(configPath);
  config.plugins.updater.pubkey=(await readFile(key+'.pub','utf8')).trim();
  await writeFile(configPath,JSON.stringify(config,null,2)+'\n');
  console.log('更新密钥已生成，公钥已写入应用配置。请将 .release-keys/updater.key 离线备份；不要提交私钥。');
}else if(command==='build'){
  const provenance=await buildProvenance();
  const config=await json(configPath);
  if(!config.plugins.updater.pubkey)throw new Error('应用缺少更新公钥');
  const privateKey=process.env.TAURI_SIGNING_PRIVATE_KEY??key;
  if(!process.env.TAURI_SIGNING_PRIVATE_KEY){
    const publicKey=(await readFile(key+'.pub','utf8')).trim();
    if(config.plugins.updater.pubkey!==publicKey)throw new Error('更新公钥与本机密钥不一致');
  }
  const version=(await json(resolve(root,'package.json'))).version;
  if(config.version!==version)throw new Error('package.json与Tauri版本不一致');
  const platform=process.platform==='darwin'?'darwin':process.platform==='win32'?'windows':null;
  if(!platform)throw new Error('请在macOS或Windows原生环境构建安装包');
  const overlay={bundle:{active:true,targets:platform==='darwin'?['app','dmg']:['nsis'],createUpdaterArtifacts:true}};
  await mkdir(resolve(root,'.release-keys'),{recursive:true,mode:0o700});
  const configFile=resolve(root,'.release-keys/build-config.json');
  await writeFile(configFile,JSON.stringify(overlay));
  // CI模式跳过Finder排版自动化，安装内容、签名及应用程序快捷入口保持。
  run(['tauri','build','--config',configFile],{...process.env,CI:process.env.CI??'true',TAURI_SIGNING_PRIVATE_KEY:privateKey,TAURI_SIGNING_PRIVATE_KEY_PASSWORD:process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD??''});
  // 构建结束重新检查源码，防止并行会话修改源码后错误沿用构建来源。
  const after=await buildProvenance();
  if(JSON.stringify(after)!==JSON.stringify(provenance))throw new Error('构建期间源码或环境发生变化');
  const bundle=resolve(root,`target/release/bundle/${platform==='darwin'?'dmg':'nsis'}`);
  const installers=(await readdir(bundle)).filter(name=>name.endsWith(platform==='darwin'?'.dmg':'.exe'));
  if(installers.length!==1)throw new Error('安装包数量不为一，请使用独立干净构建输出');
  const installer=resolve(bundle,installers[0]);
  const update=platform==='darwin'?resolve(root,'target/release/bundle/macos/WorkTwill.app.tar.gz'):installer;
  provenance.artifacts={};
  for(const path of new Set([installer,update,update+'.sig']))provenance.artifacts[path.split(/[\\/]/).pop()]=digest(await readFile(path));
  await writeFile(resolve(bundle,'build-provenance.json'),JSON.stringify(provenance,null,2)+'\n');
  // 在安装包旁附带双语说明，离线分发也能找到首次打开与更新步骤。
  await copyFile(resolve(root,'docs/INSTALLATION.txt'),resolve(bundle,'WorkTwill-INSTALL.txt'));
}else if(command==='record'){
  const result=await recordCandidate({target:option('--target'),version:option('--version'),installer:resolve(option('--installer')),update:resolve(option('--update')),provenance:resolve(option('--provenance')),notes:resolve(option('--notes-file')),output:resolve(option('--output'))});
  console.log(JSON.stringify(result,null,2));

}else{
  throw new Error('用法：pnpm release:keygen | release:build | release:record --target darwin-aarch64 --version 1.3.1 --installer file.dmg --update file.app.tar.gz --provenance build-provenance.json --notes-file notes.md --output candidate-directory');
}
