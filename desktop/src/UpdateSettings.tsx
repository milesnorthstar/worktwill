import type {UpdateStatus,UiLanguage} from './contracts';
import {errorMessage} from './api';

export function updateLabel(status:UpdateStatus,lang:UiLanguage):string {
  const en=lang==='en';
  switch(status.phase){
    case 'unconfigured':return en?'Update service is not configured yet':'更新服务尚未配置';
    case 'idle':return en?'Updates have not been checked yet':'尚未检查更新';
    case 'up_to_date':return en?'You are up to date':'当前已是最新版本';
    case 'checking':return en?'Checking for updates…':'正在检查更新…';
    case 'available':return en?`Version ${status.version} is available`:`发现新版本 ${status.version}`;
    case 'downloading':return (en?'Downloading update':'正在下载更新')+(status.total?` · ${Math.min(100,Math.floor(status.downloaded/status.total*100))}%`:` · ${(status.downloaded/1048576).toFixed(1)} MB`);
    case 'ready':return en?`Version ${status.version} is ready`:`新版本 ${status.version} 已就绪`;
    case 'installing':return en?'Installing update…':'正在安装更新…';
    case 'error':return en?'Update failed. Try again.':'更新失败，请重试';
  }
}

export default function UpdateSettings({status,language,busy,onCheck,onAutomatic,onInstall}:{status:UpdateStatus|null;language:UiLanguage;busy:boolean;onCheck:(download:boolean)=>void;onAutomatic:(enabled:boolean)=>void;onInstall:()=>void}){
  const t=(zh:string,en:string)=>language==='zh'?zh:en;
  const working=!status||['checking','downloading','installing'].includes(status.phase);
  return <section className="update-settings"><div className="settings-divider"/><h2>{t('应用更新','App updates')}</h2>
    <p>{status?`WorkTwill ${status.current_version} · ${updateLabel(status,language)}`:t('正在读取更新状态…','Reading update status…')}</p>
    {status?.error&&<p role="alert">{errorMessage(status.error,language)}</p>}
    <div className="actions"><button className="button" disabled={busy||working||status?.phase==='unconfigured'} onClick={()=>onCheck(false)}>{t('检查更新','Check for updates')}</button>
    {status?.phase==='ready'&&<button className="button primary" disabled={busy} onClick={onInstall}>{t('重启更新','Restart to update')}</button>}
    {(status?.phase==='available'||(status?.phase==='error'&&status.version))&&<button className="button primary" disabled={busy} onClick={()=>onCheck(true)}>{status.phase==='error'?t('重试下载','Retry download'):t('下载更新','Download update')}</button>}</div>
    <label className="update-preference"><input type="checkbox" checked={status?.automatic_download??false} disabled={!status||busy} onChange={event=>onAutomatic(event.target.checked)}/>{t('后台自动下载更新，安装前由我确认','Download updates in the background; ask before installing')}</label>
    <details className="update-notes"><summary>{t('安装与更新帮助','Installation and update help')}</summary><p>{t('更新包会自动校验签名。点击“重启更新”后，应用会先保存便签，再安装并重新打开。下载失败时可稍后重试。','Update packages are signature-verified automatically. Restart to update saves your notes before installing and reopening the app. If a download fails, you can retry later.')}</p><p>{t('当前版本未做系统代码签名。Mac 请将应用移入“应用程序”后运行；首次打开如提示无法验证开发者，可在“系统设置 → 隐私与安全性”中确认打开。Windows 如显示 SmartScreen 提示，可在核对官网下载来源后查看“更多信息”。','This build has no OS code signing. On Mac, move the app to Applications before running it. If the developer cannot be verified on first launch, review the app in System Settings → Privacy & Security. On Windows, if SmartScreen appears, check that the download came from the official website before reviewing More info.')}</p></details>
    {!!status?.notes&&<details className="update-notes"><summary>{t('更新说明','Release notes')}</summary><p>{status.notes}</p></details>}
  </section>;
}
