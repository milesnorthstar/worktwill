import {readFile,writeFile,mkdir,readdir,copyFile,realpath} from 'node:fs/promises';
import {dirname,join,resolve} from 'node:path';
import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';
import {run} from './release-candidate.mjs';

const root=fileURLToPath(new URL('../',import.meta.url));
const json=async path=>JSON.parse(await readFile(path,'utf8'));
async function licenseFiles(directory){
  const files=[];
  for(const entry of await readdir(directory,{withFileTypes:true})){
    if(entry.name==='node_modules'||entry.name==='.git')continue;
    const path=join(directory,entry.name);
    if(entry.isDirectory())files.push(...await licenseFiles(path));
    else if(entry.isFile()&&/^(licen[cs]e|copying|copyright|notice)([._-]|$)/i.test(entry.name))files.push(path);
  }
  return files.sort();
}

export async function distributionNotices(output){
  const target=run('rustc',['-vV']).match(/^host: (.+)$/m)[1];
  const metadata=JSON.parse(run('cargo',['metadata','--locked','--filter-platform',target,'--format-version','1']));
  const tree=run('cargo',['tree','--locked','--target',target,'-p','worktwill-desktop','--edges','normal','--prefix','none','--format','{p}']);
  // 包含目标平台的保守依赖集合及宏包；不把开发机上全部构建工具当成运行时组件。
  const selected=new Set([...tree.matchAll(/^(\S+) v(\S+)/gm)].map(match=>`${match[1]}-${match[2]}`));
  const upstream=await json(join(root,'scripts/license-sources/sources.json'));
  const records=[];
  const sections=['WorkTwill third-party notices\n\nThis file includes the target-platform dependency closure, including supporting macro crates. Unmodified MPL-covered crate sources are available at the exact version URLs listed below. These components retain their own licenses.\n'];
  for(const pkg of metadata.packages.filter(pkg=>selected.has(`${pkg.name}-${pkg.version}`)).sort((a,b)=>`${a.name}-${a.version}`.localeCompare(`${b.name}-${b.version}`))){
    if(!pkg.source)continue;
    const name=`${pkg.name}-${pkg.version}`;
    const source=`https://crates.io/api/v1/crates/${pkg.name}/${pkg.version}/download`;
    const directory=dirname(pkg.manifest_path);
    const files=await licenseFiles(directory);
    const texts=await Promise.all(files.map(async path=>`${path.slice(directory.length+1)}\n${await readFile(path,'utf8')}`));
    for(const entry of upstream[name]??[])texts.push(`${entry.url}\n${await readFile(join(root,'scripts/license-sources',entry.file),'utf8')}`);
    if(!texts.length)throw new Error(`缺少第三方许可原文：${name}`);
    records.push({name:pkg.name,version:pkg.version,license:pkg.license,source});
    sections.push(`\n${'='.repeat(72)}\n${name}\nLicense: ${pkg.license}\nSource: ${source}\n\n${texts.join('\n\n')}`);
  }
  const npmSeen=new Set();
  async function npmPackage(directory){
    directory=await realpath(directory);
    if(npmSeen.has(directory))return;
    npmSeen.add(directory);
    const pkg=await json(join(directory,'package.json'));
    const files=await licenseFiles(directory);
    if(!files.length)throw new Error(`缺少第三方许可原文：${pkg.name}@${pkg.version}`);
    records.push({name:pkg.name,version:pkg.version,license:pkg.license,source:`https://www.npmjs.com/package/${pkg.name}/v/${pkg.version}`});
    sections.push(`\n${'='.repeat(72)}\n${pkg.name}@${pkg.version}\nLicense: ${pkg.license}\n\n${(await Promise.all(files.map(async path=>`${path.slice(directory.length+1)}\n${await readFile(path,'utf8')}`))).join('\n\n')}`);
    const require=createRequire(join(directory,'package.json'));
    for(const name of Object.keys(pkg.dependencies??{})){
      let manifest;
      try{manifest=require.resolve(`${name}/package.json`);}catch(error){
        if(error.code!=='ERR_PACKAGE_PATH_NOT_EXPORTED')throw error;
        let location=dirname(require.resolve(name));
        while((await json(join(location,'package.json'))).name!==name)location=dirname(location);
        manifest=join(location,'package.json');
      }
      await npmPackage(dirname(manifest));
    }
  }
  for(const name of Object.keys((await json(join(root,'package.json'))).dependencies))await npmPackage(join(root,'node_modules',name));
  await mkdir(output,{recursive:true});
  await writeFile(join(output,'THIRD-PARTY.txt'),sections.join('\n'));
  await writeFile(join(output,'components.json'),JSON.stringify({target,components:records},null,2)+'\n');
  await copyFile(join(root,'LICENSE'),join(output,'WorkTwill-LICENSE.txt'));
  // Rust标准库静态链接到主程序；携带工具链提供的库版权和完整许可原文。
  const rustDocs=join(run('rustc',['--print','sysroot']),'share/doc/rust');
  await copyFile(join(rustDocs,'COPYRIGHT-library.html'),join(output,'Rust-library-COPYRIGHT.html'));
  await mkdir(join(output,'rust-licenses'),{recursive:true});
  for(const file of await readdir(join(rustDocs,'licenses')))await copyFile(join(rustDocs,'licenses',file),join(output,'rust-licenses',file));
  return {target,components:records.length};
}
if(process.argv[1]&&resolve(process.argv[1])===fileURLToPath(import.meta.url))console.log(await distributionNotices(resolve(process.argv[2])));
