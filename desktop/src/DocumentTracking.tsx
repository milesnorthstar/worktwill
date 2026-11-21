import {useEffect,useState} from 'react';
import {api,errorMessage} from './api';
import type {DocumentTrackingStatus,Overview,UiLanguage} from './contracts';

export default function DocumentTracking({roots,revision,language,busy,run}:{roots:Overview['roots'];revision:number;language:UiLanguage;busy:boolean;run:(task:()=>Promise<unknown>)=>Promise<unknown>}) {
  const [status,setStatus]=useState<DocumentTrackingStatus|null>(null);
  const [error,setError]=useState<unknown>(null);
  const [consent,setConsent]=useState<string|null>(null);
  const t=(zh:string,en:string)=>language==='zh'?zh:en;
  useEffect(()=>{let alive=true;api.documentTracking().then(value=>{if(alive){setStatus(value);setError(null);}}).catch(cause=>{if(alive)setError(cause);});return()=>{alive=false;};},[revision,roots]);
  if(error)return <p role="alert">{errorMessage(error,language)}</p>;
  if(!status?.supported)return null;
  return <section className="document-tracking"><div className="settings-divider"/><h2>{t('文档保存追踪','Document save tracking')}</h2>
    <p className="muted">{t('仅对明确启用的 NTFS 目录，为收藏或工作区文件按需写入系统追踪标记，不修改正文。复制及另存副本保持独立；系统明确覆盖可延续原关联。','For explicitly enabled NTFS folders, system tracking markers are added to favorites and workspace files without editing content. Copies remain separate; system replacement can retain references.')}</p>
    {roots.filter(root=>root.active).map(root=>{const enabled=status.roots.some(item=>item.root_id===root.id&&item.enabled);return <div className="location-card" key={root.id}><strong>{root.path}</strong><p>{enabled?t('已允许追踪；逐文件结果见下方','Tracking allowed; see individual results below'):t('未启用，不写入追踪标记','Disabled; no tracking markers are written')}</p>
      {consent===root.id?<div><p>{t('允许为此目录的重要文件创建 NTFS Object ID？关闭追踪或撤权不会删除已有标记。','Allow NTFS Object IDs for important files in this folder? Disabling tracking or revoking access will not delete existing markers.')}</p><button className="button primary" disabled={busy} onClick={()=>{void run(()=>api.setDocumentTracking(root.id,true));setConsent(null);}}>{t('同意并启用追踪','Agree and enable tracking')}</button><button className="button" onClick={()=>setConsent(null)}>{t('取消','Cancel')}</button></div>:<button className="button" disabled={busy||(!enabled&&root.paused)} onClick={()=>{if(enabled)void run(()=>api.setDocumentTracking(root.id,false));else setConsent(root.id);}}>{enabled?t('关闭追踪','Disable tracking'):t('启用保存追踪','Enable save tracking')}</button>}
    </div>;})}
    {status.files.map(file=><p key={file.resource_id}><strong>{file.name}</strong> · {file.error?errorMessage(file.error,language):file.established?t('追踪标记已建立','Tracking marker established'):t('尚未建立追踪标记','Tracking marker pending')}</p>)}
  </section>;
}
