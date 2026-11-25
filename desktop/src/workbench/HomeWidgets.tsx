import {useCallback,useEffect,useRef,useState} from 'react';
import {isTauri} from '@tauri-apps/api/core';
import {getCurrentWindow} from '@tauri-apps/api/window';
import {listen} from '@tauri-apps/api/event';
import {Plus,Pin,Trash2,X} from 'lucide-react';
import {api,errorMessage,native} from '../api';
import type {ScratchNote,UiLanguage} from '../contracts';
import companion from '../assets/space-companion.svg';

type Draft={scope:string;note:ScratchNote;text:string;pinned:boolean;deleted:boolean;saving:boolean;error:unknown|null;confirmed:boolean};
export type NotesExitGuard={current:(()=>Promise<boolean>)|null};
export default function HomeWidgets({lang,visible,exitGuard}:{lang:UiLanguage;visible:boolean;exitGuard?:NotesExitGuard}){
  const t=(zh:string,en:string)=>lang==='zh'?zh:en;
  const [drafts,setDrafts]=useState<Draft[]>([]);
  const current=useRef(drafts);current.current=drafts;
  const scope=useRef('');
  const readSequence=useRef(0);
  const deletedIds=useRef(new Set<string>());
  const [loadError,setLoadError]=useState<unknown>(null);
  const [all,setAll]=useState(false);
  const [showBuddy,setShowBuddy]=useState(true);
  const allowingClose=useRef(false);
  const writes=useRef(new Map<string,Promise<boolean>>());
  const update=useCallback((id:string,fn:(draft:Draft)=>Draft)=>{const next=current.current.map(d=>d.note.id===id?fn(d):d);current.current=next;setDrafts(next);},[]);
  const read=useCallback(async()=>{
    const sequence=++readSequence.current;
    try{
      const response=await api.notes();
      if(sequence!==readSequence.current)return;
      if(scope.current!==response.scope)deletedIds.current.clear();
      scope.current=response.scope;setLoadError(null);
      setDrafts(old=>{
        const next=response.items.filter(note=>!deletedIds.current.has(note.id)).map(note=>{
          const local=old.find(d=>d.note.id===note.id);
          if(local&&local.scope!==response.scope&&(local.saving||dirty(local)))return {...local,error:{code:'conflict',message:'数据已恢复，原草稿尚未提交。可保留为新便签。',message_en:'Data was restored. Keep this draft as a new note.'}};
          return local&&(local.note.version>note.version||local.saving||!local.confirmed||local.text!==local.note.text||local.pinned!==local.note.pinned||local.deleted)?local:{scope:response.scope,note,text:note.text,pinned:note.pinned,deleted:false,saving:false,error:null,confirmed:true};
        });
        for(const local of old)if(!next.some(d=>d.note.id===local.note.id)&&(!local.confirmed||local.saving||local.text!==local.note.text||local.deleted))next.push(local);
        current.current=next;return next;
      });
    }catch(cause){if(sequence===readSequence.current)setLoadError(cause);}
  },[]);
  const dirty=(d:Draft)=>!d.confirmed||d.text!==d.note.text||d.pinned!==d.note.pinned||d.deleted;
  const save=useCallback((id:string):Promise<boolean>=>{
    const active=writes.current.get(id);
    if(active)return active;
    let d=current.current.find(d=>d.note.id===id);
    if(!d||!dirty(d))return Promise.resolve(true);
    update(id,d=>({...d,saving:true,error:null}));
    // 退出与自动保存等待同一条串行任务，不重复提交，也不在在途时默默放弃退出。
    const pending=(async()=>{try{
      // 每个 UUID 串行提交快照；保存中继续编辑只改变草稿，不显示为已保存。
      while(d&&dirty(d)){
        const snapshot={...d};
        const saved=await api.writeNote({scope:snapshot.scope,id,expected_version:snapshot.note.version,text:snapshot.text,pinned:snapshot.pinned,deleted:snapshot.deleted});
        if(snapshot.deleted){deletedIds.current.add(id);const next=current.current.filter(item=>item.note.id!==id);current.current=next;setDrafts(next);return true;}
        update(id,latest=>({...latest,note:saved,confirmed:true}));
        d=current.current.find(item=>item.note.id===id);
      }
      update(id,d=>({...d,saving:false}));return true;
    }catch(cause){update(id,d=>({...d,saving:false,error:cause}));return false;}})().finally(()=>{writes.current.delete(id);});
    writes.current.set(id,pending);
    return pending;
  },[update]);
  const flush=useCallback(async()=>{
    const pending=current.current.filter(d=>dirty(d)||d.saving);
    const results=await Promise.all(pending.map(d=>save(d.note.id)));
    return results.every(Boolean)&&!current.current.some(d=>dirty(d)||d.saving);
  },[save]);
  // 更新与关闭复用同一个保存守卫，草稿仍归小组件持有。
  useEffect(()=>{if(!exitGuard)return;exitGuard.current=flush;return()=>{exitGuard.current=null;};},[exitGuard,flush]);
  useEffect(()=>{if(!native)return;void read();let alive=true;const subscription=listen('notes-changed',()=>{if(alive)void read();});const restore=listen('data-restored',()=>{if(alive)void read();});return()=>{alive=false;void subscription.then(fn=>fn());void restore.then(fn=>fn());};},[read]);
  useEffect(()=>{
    const timer=window.setTimeout(()=>{for(const d of current.current)if(dirty(d)&&!d.saving&&!d.error)void save(d.note.id);},500);
    return()=>window.clearTimeout(timer);
  },[drafts,save]);
  useEffect(()=>{
    const before=(event:BeforeUnloadEvent)=>{if(current.current.some(d=>dirty(d)||d.saving)){event.preventDefault();event.returnValue='';}};
    window.addEventListener('beforeunload',before);
    if(!isTauri())return()=>window.removeEventListener('beforeunload',before);
    let alive=true;
    const subscription=getCurrentWindow().onCloseRequested(async event=>{
      if(allowingClose.current||!current.current.some(d=>dirty(d)||d.saving))return;
      event.preventDefault();
      if(await flush()&&alive){allowingClose.current=true;await getCurrentWindow().close();}
    });
    return()=>{alive=false;window.removeEventListener('beforeunload',before);void subscription.then(fn=>fn());};
  },[flush]);
  const sorted=[...drafts].sort((a,b)=>Number(b.pinned)-Number(a.pinned)||b.note.updated_at-a.note.updated_at);
  return <>{!visible&&drafts.some(d=>d.error!==null)&&<div className="note-exit-error" role="alert">{t('便签保存失败，草稿仍在。请返回个人工作台重试，应用尚未退出。','A note could not be saved. Return to the workbench to retry. The app remains open.')}</div>}<aside className="home-widgets" hidden={!visible} aria-label={t('个人小组件','Personal widgets')}>
    <section className="companion-card">
      <div className="section-heading"><h2>🌱 {t('空间伙伴','Space companion')}</h2><button className="text-button" aria-expanded={showBuddy} onClick={() => setShowBuddy(value => !value)}>{showBuddy ? t('收起','Hide') : t('展开','Show')}</button></div>
      <div className="companion-reveal" data-expanded={showBuddy} inert={!showBuddy}>
        <div><img src={companion} alt={t('盆栽空间伙伴','A friendly potted plant')}/><p>{t('留一个安静的角落，继续手头的工作。','A quiet corner for the work at hand.')}</p></div>
      </div>
    </section>
    <section className="notes-card"><div className="section-heading"><h2>{t('工作便签','Scratch notes')}</h2><button className="icon-button" aria-label={t('新建便签','New note')} disabled={!native||!scope.current} onClick={()=>{const note={id:crypto.randomUUID(),text:'',pinned:false,version:0,updated_at:Date.now()};setDrafts(old=>{const next=[{scope:scope.current,note,text:'',pinned:false,deleted:false,saving:false,error:null,confirmed:false},...old];current.current=next;return next;});}}><Plus size={20}/></button></div>
    {loadError!==null&&<p role="alert">{errorMessage(loadError,lang)} <button className="text-button" onClick={()=>void read()}>{t('重新读取','Reload')}</button></p>}
    {!drafts.length&&<p className="muted">{t('随手记下，不必先建工作区。','Jot something down, with no workspace required.')}</p>}
    {(all?sorted:sorted.slice(0,3)).map(d=><article key={d.note.id} className="scratch-note"><textarea aria-label={t('便签正文 ','Note text ')+d.note.id} value={d.text} maxLength={20000} disabled={d.deleted} placeholder={t('写点什么…','Write a note…')} onChange={event=>update(d.note.id,old=>({...old,text:event.target.value,error:null}))}/><div className="note-actions"><span role="status">{d.error?t('保存失败，草稿未提交','Save failed. Draft not committed'):d.saving||dirty(d)?t('保存中…','Saving…'):t('已保存','Saved')}</span>{d.error!==null&&<button className="text-button" onClick={()=>void save(d.note.id)}>{t('重试','Retry')}</button>}<button className="icon-button" disabled={d.deleted} aria-label={d.pinned?t('取消置顶','Unpin'):t('置顶','Pin')} aria-pressed={d.pinned} onClick={()=>update(d.note.id,old=>({...old,pinned:!old.pinned,error:null}))}><Pin size={14}/></button><button className="icon-button" disabled={d.deleted} aria-label={t('删除便签','Delete note')} onClick={()=>{update(d.note.id,old=>({...old,deleted:true,error:null}));void save(d.note.id);}}><Trash2 size={14}/></button></div>{d.error!==null&&<p role="alert">{errorMessage(d.error,lang)}{typeof d.error==='object'&&'code' in d.error&&d.error.code==='conflict'&&<button className="text-button" onClick={()=>{const note={id:crypto.randomUUID(),text:'',pinned:false,version:0,updated_at:Date.now()};const next=current.current.filter(item=>item.note.id!==d.note.id);next.unshift({scope:scope.current,note,text:d.text,pinned:d.pinned,deleted:false,saving:false,error:null,confirmed:false});current.current=next;setDrafts(next);}}>{t('保留为新便签','Keep as new note')}</button>}</p>}</article>)}
    <button className="text-button" onClick={()=>setAll(value=>!value)}>{all?<><X size={14}/>{t('收起全部便签','Collapse notes')}</>:t('查看全部便签','View all notes')}</button><p className="note-promise">{t('已保存内容重启可取回。保存失败时请重试；强制退出可能丢失未提交草稿。','Saved notes survive restart. Retry failed saves; forcing quit can lose uncommitted drafts.')}</p></section>
  </aside></>;
}
