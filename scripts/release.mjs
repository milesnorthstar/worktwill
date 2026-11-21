import {spawnSync} from 'node:child_process';
import {readFile,writeFile,mkdir,chmod,copyFile,access} from 'node:fs/promises';
import {resolve,basename} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {platformNames} from '../website/release.mjs';

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
  // 在安装包旁附带双语说明，离线分发也能找到首次打开与更新步骤。
  await copyFile(resolve(root,'docs/INSTALLATION.txt'),resolve(root,`target/release/bundle/${platform==='darwin'?'dmg':'nsis'}/WorkTwill-INSTALL.txt`));
}else if(command==='record'){
  const target=option('--target');
  if(!platformNames[target])throw new Error('不支持的目标平台');
  const version=option('--version');
  if(!/^\d+\.\d+\.\d+(?:-[\w.-]+)?$/.test(version))throw new Error('版本必须为SemVer');
  const installer=resolve(option('--installer'));
  const update=resolve(option('--update'));
  if(target.startsWith('darwin')&&(!installer.endsWith('.dmg')||!update.endsWith('.app.tar.gz')))throw new Error('Mac安装需要DMG，更新需要app.tar.gz');
  if(target.startsWith('windows')&&(!installer.endsWith('.exe')||!update.endsWith('.exe')))throw new Error('Windows需要NSIS安装EXE');
  const signature=(await readFile(update+'.sig','utf8')).trim();
  const decoded=Buffer.from(signature,'base64').toString('utf8');
  if(!decoded.split(/\r?\n/).find(line=>line.startsWith('trusted comment:'))?.split('\t').includes(`version:${version}`))throw new Error('更新签名未绑定所登记版本');
  const base=option('--base-url',false);
  if(base&&new URL(base).protocol!=='https:')throw new Error('分发域名必须使用HTTPS');
  await mkdir(resolve(root,'deliverables'),{recursive:true});
  async function artifact(path){
    const bytes=await readFile(path);
    const sha256=createHash('sha256').update(bytes).digest('hex');
    const filename=`WorkTwill-${version}-${target}-${sha256.slice(0,12)}-${basename(path)}`;
    const destination=resolve(root,'deliverables',filename);
    if(await exists(destination)){
      const previous=await readFile(destination);
      if(createHash('sha256').update(previous).digest('hex')!==sha256)throw new Error('已登记同名制品内容不同，拒绝覆盖');
    }else await copyFile(path,destination);
    return {filename,bytes:bytes.length,sha256,...(base?{url:new URL(filename,base.endsWith('/')?base:base+'/').href}:{})};
  }
  const record={version,stage:'本地安装与更新候选',stage_en:'Local installation and update candidate',published_at:new Date().toISOString(),notes:option('--notes-file',false)?await readFile(resolve(option('--notes-file')),'utf8'):'',...await artifact(installer),update:{...await artifact(update),signature}};
  const registryPath=resolve(root,'website/release.json');
  const registry=await json(registryPath);
  registry.platforms[target]=record;
  await writeFile(registryPath,JSON.stringify(registry,null,2)+'\n');
  console.log(`已登记 ${target} ${version}。先上传并核验制品，再发布官网及更新清单。`);
}else{
  throw new Error('用法：pnpm release:keygen | release:build | release:record --target darwin-aarch64 --version 1.3.1 --installer file.dmg --update file.app.tar.gz [--base-url https://downloads.example.com/版本/] [--notes-file notes.md]');
}
