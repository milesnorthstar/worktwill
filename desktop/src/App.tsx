import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { ArrowLeft, ArrowRight, Check, Clock3, ChevronRight, Folder, Home, Layers3, Link2, LoaderCircle, Plus, RefreshCw, Search, Settings2, ShieldCheck, Star, Trash2, X } from 'lucide-react';
import { api, errorMessage, native, nextSearchRequestId } from './api';
import type { Mutation, Overview, Resource, ResourcePage, ImportResult, SearchCursor, UiLanguage } from './contracts';
import ActivityTimeline from './timeline/ActivityTimeline';
import { useResourceDrag } from './workbench/useResourceDrag';
import { EmptyState, ResourceList, SearchPanel, SearchFilters, LocationSuggestions } from './workbench/components';

type View = 'home' | 'files' | 'workspaces' | 'workspace' | 'activity' | 'settings' | 'search';
type SearchOrigin = {view:Exclude<View,'search'>;workspace:string|null;mode:string;kind:string};
type Lang = UiLanguage;
type Failure = { cause: unknown; source?: 'page' } | null;
type FormState = { kind: 'workspace' | 'rename' | 'web' | 'link' | 'transfer' | 'delete' | 'deleteweb'; resourceIds?: string[]; id?: string; name?: string; url?: string };

function webUrl(value:string) {
  const trimmed=value.trim();
  // 粘贴裸域名时补全 HTTPS；已有协议交给原生校验，不把其他协议伪装成网址。
  return trimmed&&!/^[a-z][a-z\d+.-]*:/i.test(trimmed)?`https://${trimmed}`:trimmed;
}

function cancelSearch(lane:'quick'|'page',id:number) {
  void api.cancelSearch(lane,id).catch(cause=>{
    // 旧 WebView 会话已被新会话取消是预期结果；其他取消错误保留诊断，不伪装查询成功。
    if(!(typeof cause==='object'&&cause!==null&&'code' in cause&&cause.code==='search_cancelled'))console.warn('搜索取消失败',cause);
  });
}

export default function App({initialView='home'}:{initialView?:View}) {
  const [dropResult,setDropResult]=useState<{token:string;workspace:string|null;result:ImportResult}|null>(null);
  const [externalDrag,setExternalDrag]=useState(false);
  const [dropPreparing,setDropPreparing]=useState(false);
  const [batchReport,setBatchReport]=useState<{succeeded:number;failures:Array<{name:string|null;cause:unknown}>}|null>(null);
  const [lang, setLang] = useState<Lang>('zh');
  const currentLanguage = useRef(lang); currentLanguage.current=lang;
  const t = useCallback((zh: string, en: string) => lang === 'zh' ? zh : en, [lang]);
  const [view, setView] = useState<View>(initialView);
  const [navigation, setNavigation] = useState(0);
  const [workspace, setWorkspace] = useState<string | null>(null);
  const [overview, setOverview] = useState<Overview | null>(null);
  const [page, setPage] = useState<ResourcePage | null>(null);
  const [mode, setMode] = useState('modified');
  const [kind, setKind] = useState('all');
  const [sort,setSort]=useState('relevance');
  const [searchFilters,setSearchFilters]=useState({from:'',to:'',path:''});
  const [excludedFolders,setExcludedFolders]=useState<Record<string,string>>({});
  const [offset, setOffset] = useState(0);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [query, setQuery] = useState('');
  const [searchOpen, setSearchOpen] = useState(false);
  const [searchOrigin,setSearchOrigin]=useState<SearchOrigin|null>(null);
  const [defaultActive,setDefaultActive]=useState(-1);
  const [searchScope, setSearchScope] = useState<'all' | 'current'>('all');
  const [addIntent, setAddIntent] = useState(false);
  const [results, setResults] = useState<Resource[]>([]);
  const [searching, setSearching] = useState(false);
  const [active, setActive] = useState(-1);
  const [form, setForm] = useState<FormState | null>(null);
  const [notice, setNotice] = useState('');
  // 错误保留原始双语内容，切换语言时只更新显示，不重新提交或查询。
  const [error, setError] = useState<Failure>(null);
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);
  const [refresh, setRefresh] = useState(0);
  const [loading, setLoading] = useState(true);
  const [reloadHeight,setReloadHeight]=useState<number>();
  const mainArea=useRef<HTMLElement>(null);
  const searchInput = useRef<HTMLInputElement>(null);
  const searchArea = useRef<HTMLDivElement>(null);
  const searchSequence = useRef(0);
  const loadSequence = useRef(0);
  const nextSearchCursor = useRef<SearchCursor | null>(null);
  const currentPermission = useRef({ session: '', version: 0, revision: 0 });
  useEffect(() => { setError(null); }, [form?.kind, form?.id]);
  // 页面提交后再回顶；仅导航递增，分页、轮询和页内编辑不会打断滚动。
  useLayoutEffect(() => { setReloadHeight(undefined);window.scrollTo(0,0); }, [navigation]);
  const dropSequence=useRef(0);
  const startedDrop=useRef<{token:string;workspace:string|null;sequence:number}|null>(null);
  const currentDropWorkspace=useRef(workspace); currentDropWorkspace.current=view==='workspace'?workspace:null;
  const currentWorkspace = overview?.workspaces.find(w => w.id === workspace);

  const reload = useCallback(() => {
    // 清除过期授权文本，但保留几何高度，避免列表暂空使浏览器把滚动位置挤回顶部。
    setReloadHeight(mainArea.current?.getBoundingClientRect().height);setLoading(true);
    searchSequence.current++;
    loadSequence.current++;
    nextSearchCursor.current = null;
    setResults([]); setOverview(null); setSelected(new Set()); setPage(null); setOffset(0); setRefresh(v => v + 1);
  }, []);

  useEffect(() => {
    if (!native) { setLoading(false); return; }
    let alive = true;
    const clearDropHighlight=()=>document.querySelectorAll('.native-drop-target').forEach(element=>element.classList.remove('native-drop-target'));
    const subscriptions = [
      listen('catalog-changed', () => { if (alive) reload(); }),
      listen('scan-progress',()=>{
        const sequence=loadSequence.current;
        // 无内容变化的扫描只更新进度；真实版本变化仍清旧页，迟到进度不能恢复旧授权文本。
        void api.overview().then(meta=>{
          if(!alive||sequence!==loadSequence.current)return;
          const current=currentPermission.current;
          if(meta.session!==current.session||meta.permission_version!==current.version||meta.revision!==current.revision)reload();
          else setOverview(meta);
        }).catch(cause=>{if(alive&&sequence===loadSequence.current)setError({cause});});
      }),
      listen<{token:string;x:number;y:number;count:number}>('files-drop-started',event=>{
        if(!alive)return;
        const point=event.payload;
        const target=document.elementFromPoint(point.x/window.devicePixelRatio,point.y/window.devicePixelRatio)?.closest<HTMLElement>('[data-workspace-id]');
        // 松手时固定目标；慢盘身份确认期间导航或滚动不得把资料加入另一工作区。
        startedDrop.current={token:point.token,workspace:target?.dataset.workspaceId??currentDropWorkspace.current,sequence:++dropSequence.current};
        setDropPreparing(true);setDropResult(null);setExternalDrag(false);clearDropHighlight();
      }),
      listen<{x:number;y:number;count?:number}>('files-dragging',event=>{
        if(!alive)return;
        setExternalDrag(true);clearDropHighlight();
        document.elementFromPoint(event.payload.x/window.devicePixelRatio,event.payload.y/window.devicePixelRatio)?.closest('[data-workspace-id]')?.classList.add('native-drop-target');
      }),
      listen('files-drag-left',()=>{if(alive)setExternalDrag(false);clearDropHighlight();}),
      listen<{token:string;count:number;x:number;y:number}>('files-dropped', event => {
        if(!alive)return;
        const point=event.payload;
        const snapshot=startedDrop.current;
        if(!snapshot||snapshot.token!==point.token)return;
        startedDrop.current=null;
        setDropPreparing(false);setExternalDrag(false);clearDropHighlight();
        const sequence=snapshot.sequence;
        const wid=snapshot.workspace;
        void api.importDrop(point.token,wid,false,currentLanguage.current).then(result=>{if(alive&&sequence===dropSequence.current){setDropResult({token:point.token,workspace:wid,result});reload();}}).catch(cause=>{if(alive&&sequence===dropSequence.current)setError({cause});});
      }),
      listen('operation-error' , event => { if (alive) {setDropPreparing(false);setError({cause:event.payload});} }),
    ];
    return () => { alive = false; clearDropHighlight(); for (const promise of subscriptions) void promise.then(unlisten => unlisten()); };
  }, [reload]);

  useEffect(() => {
    if (!native) return;
    const sequence = ++loadSequence.current;
    let requestId:number|null=null;
    setLoading(true);
    void (async () => {
      try {
        const meta = await api.overview();
        if (sequence !== loadSequence.current) return;
        currentPermission.current = { session: meta.session, version: meta.permission_version, revision:meta.revision };
        setOverview(meta);
        const dateEnd=searchFilters.to?new Date(`${searchFilters.to}T00:00:00`):null;
        if(dateEnd)dateEnd.setDate(dateEnd.getDate()+1);
        if(view==='search')requestId=nextSearchRequestId();
        const response = view==='search' ? await api.searchPage({request:{request_id:requestId!,query,workspace_id:searchScope==='current'?workspace:null,kind},offset,sort,cursor:offset>0?nextSearchCursor.current??undefined:undefined,modified_from:searchFilters.from?new Date(`${searchFilters.from}T00:00:00`).getTime():undefined,modified_to:dateEnd?.getTime(),path_prefix:searchFilters.path||undefined}):null;
        if(response&&response.request_id!==requestId)throw {message:'搜索响应标识不匹配，请重试',message_en:'Search response mismatch. Please retry.'};
        const data = view==='search'?response!.page:await api.resources({ workspace_id: view === 'workspace' ? workspace : null, kind, mode, offset });
        if (sequence !== loadSequence.current) return;
        if (data.session !== meta.session || data.permission_version !== meta.permission_version) { reload(); return; }
        if(offset>0&&page&&(data.revision!==page.revision||data.session!==page.session||data.permission_version!==page.permission_version)){setPage(null);setSelected(new Set());setOffset(0);return;}
        if(response){
          if(data.has_more&&!response.next_cursor)throw {message:'搜索分页信息不完整，请重试',message_en:'Search pagination is incomplete. Please retry.'};
          nextSearchCursor.current=response.next_cursor??null;
        }
        currentPermission.current.revision=data.revision;
        // 只有本轮成功的列表查询可消除旧查询错误，不能吞掉并行操作失败。
        setError(previous => previous?.source === 'page' ? null : previous);
        setPage(old => offset > 0 && old ? { ...data, items: [...old.items, ...data.items.filter(item=>!old.items.some(previous=>previous.id===item.id))] } : data);
      } catch (cause) {
        if (sequence !== loadSequence.current) return;
        if(view==='search'&&offset>0&&typeof cause==='object'&&cause!==null&&'code' in cause&&cause.code==='stale_cursor'){
          // 失效页不能拼进旧列表；只重读首页一次，首页失败仍明确显示错误。
          nextSearchCursor.current=null;setPage(null);setSelected(new Set());setOffset(0);setError(null);
          setNotice(t('资料已更新，正在重新加载搜索结果','Resources changed. Reloading search results.'));
        }else setError({cause,source:'page'});
      }
      finally { if (sequence === loadSequence.current) setLoading(false); }
    })();
    return()=>{if(sequence===loadSequence.current)loadSequence.current++;if(requestId!==null)cancelSearch('page',requestId);};
  }, [view, workspace, kind, mode, offset, refresh, reload, sort, searchScope, searchFilters, view==='search'?query:null]);

  useEffect(() => {
    if (!overview?.roots.some(root => root.scan_state === 'running')) return;
    const timer = window.setInterval(() => { const sequence=loadSequence.current;void api.overview().then(meta => { if(sequence!==loadSequence.current)return;if(meta.session===currentPermission.current.session && meta.permission_version===currentPermission.current.version) setOverview(meta); else reload(); }).catch(cause=>{if(sequence===loadSequence.current)setError({cause});}); }, 1200);
    return () => window.clearInterval(timer);
  }, [overview?.roots]);

  useEffect(() => {
    const sequence = ++searchSequence.current;
    setResults([]); setActive(-1); setDefaultActive(-1);
    if (!native || !searchOpen || !query.trim()) { setSearching(false); return; }
    setSearching(true);
    let requestId:number|null=null;
    const timer = window.setTimeout(() => {
      requestId=nextSearchRequestId();
      void api.search({ request_id: requestId, query, workspace_id: !addIntent && searchScope === 'current' ? workspace : null, kind: addIntent ? 'file' : 'all' })
        .then(response => {
          const permission = currentPermission.current;
          if (sequence !== searchSequence.current || response.request_id !== requestId) return;
          if (response.page.session !== permission.session || response.page.permission_version !== permission.version) { reload(); return; }
          const order = ['workspace', 'file', 'web'];
          const items=response.page.items;
          const grouped=[...items].sort((a,b)=>order.indexOf(a.kind)-order.indexOf(b.kind));
          // 展示分组不改变后端首选；同名首选等待用户明确选择，避免猜测原件。
          const ambiguous=items.length>1&&items[0].name.normalize('NFC').toLowerCase()===items[1].name.normalize('NFC').toLowerCase();
          setDefaultActive(items.length&&!ambiguous?grouped.findIndex(item=>item.id===items[0].id):-1);
          setResults(grouped);
        }).catch(cause => { if (sequence === searchSequence.current) setError({cause}); })
        .finally(() => { if (sequence === searchSequence.current) setSearching(false); });
    }, 40);
    return () => {window.clearTimeout(timer);if(sequence===searchSequence.current)searchSequence.current++;if(requestId!==null)cancelSearch('quick',requestId);};
  }, [query, searchOpen, searchScope, workspace, addIntent, refresh, reload]);

  useEffect(() => {
    const handle = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k' && !document.querySelector('dialog[open]')) { event.preventDefault(); searchInput.current?.focus(); searchInput.current?.select(); setSearchOpen(true); setAddIntent(false); }
      if (event.key === 'Escape' && !event.isComposing) { searchSequence.current++; setSearchOpen(false); setForm(null); setAddIntent(false); }
    };
    const outside = (event: MouseEvent) => { if (!searchArea.current?.contains(event.target as Node)) {setSearchOpen(false);setAddIntent(false);} };
    window.addEventListener('keydown', handle); window.addEventListener('mousedown', outside);
    return () => { window.removeEventListener('keydown', handle); window.removeEventListener('mousedown', outside); };
  }, []);
  useEffect(() => { if (!notice) return; const timer = window.setTimeout(() => setNotice(''), 3500); return () => window.clearTimeout(timer); }, [notice]);

  async function run<T,>(task: () => Promise<T>, message?: string): Promise<T | undefined> {
    if (busyRef.current) return;
    busyRef.current = true;
    setBusy(true); setError(null);
    try { const value = await task(); if (message) setNotice(message); reload(); return value; }
    catch (cause) { setError({cause}); }
    finally { busyRef.current = false; setBusy(false); }
  }
  const {drag,onStart:startResourceDrag}=useResourceDrag((ids,wid)=>{void saveForm({type:'link',workspace_id:wid,resource_ids:ids});});
  const mutate = (mutation: Mutation, message?: string) => run(() => api.mutate(mutation), message);
  async function saveForm(mutation:Mutation) {
    if((mutation.type==='link'||mutation.type==='transfer')&&mutation.resource_ids.length>1){
      // 每份引用独立提交；某项失效不会撤回其他已成功的关联，也不删除失败项的来源。
      const names=new Map(page?.items.map(item=>[item.id,item.status==='permission_denied'?null:item.name]));
      const report=await run(async()=>{
        const failures:Array<{name:string|null;cause:unknown}>=[];let succeeded=0;
        for(const id of mutation.resource_ids){
          try{await api.mutate({...mutation,resource_ids:[id]});succeeded++;}
          catch(cause){failures.push({name:names.get(id)??null,cause});}
        }
        return{succeeded,failures};
      });
      if(report){setBatchReport(report);setForm(null);}return;
    }
    const result=await mutate(mutation,t('已保存','Saved'));
    if(result){setForm(null);if(mutation.type==='delete_workspace')navigate('workspaces');else if(mutation.type==='create_workspace'&&mutation.resource_ids?.length&&result.id)navigate('workspace',result.id);}
  }
  function navigate(next: View, id: string | null = null) {
    setNavigation(value=>value+1);
    setSearchOrigin(null);
    setError(null);
    loadSequence.current++; searchSequence.current++;
    setView(next); setWorkspace(id); setPage(null); setOffset(0); setKind('all'); setMode('modified'); setSelected(new Set()); setSearchOpen(false); setAddIntent(false); setQuery(''); setSearchScope('all');setSearchFilters({from:'',to:'',path:''});
  }
  function showSearchResults() {
    if(view!=='search')setSearchOrigin({view,workspace,mode,kind});
    setNavigation(value=>value+1);setView('search');setPage(null);setOffset(0);setKind('all');setSelected(new Set());setSearchOpen(false);setAddIntent(false);
  }
  function returnFromSearch() {
    if(!searchOrigin)return;
    const target=searchOrigin;
    if(target.view==='workspace'&&overview&&!overview.workspaces.some(item=>item.id===target.workspace)){navigate('workspaces');return;}
    navigate(target.view,target.workspace);setMode(target.mode);setKind(target.kind);
  }
  const displayedActive=active>=0?active:defaultActive;
  const returnLabel=searchOrigin?.view==='workspace' ? overview&&!overview.workspaces.some(item=>item.id===searchOrigin.workspace) ? t('返回工作区列表','Back to workspaces') : t('返回：','Back to ')+(overview?.workspaces.find(item=>item.id===searchOrigin.workspace)?.name??t('工作区','Workspace')) : searchOrigin?.view==='home'?t('返回个人工作台','Back to personal workbench'):searchOrigin?.view==='files'?searchOrigin.mode==='favorites'?t('返回收藏','Back to favorites'):t('返回所有文件','Back to all files'):searchOrigin?.view==='activity'?t('返回动态','Back to activity'):searchOrigin?.view==='settings'?t('返回设置','Back to settings'):t('返回工作区列表','Back to workspaces');
  async function activate(resource: Resource) {
    if (resource.kind === 'workspace') { navigate('workspace', resource.id); return; }
    if (addIntent && workspace) { await mutate({ type: 'link', workspace_id: workspace, resource_ids: [resource.id] }, t('已加入工作区', 'Added to workspace')); return; }
    setSearchOpen(false); await run(() => api.open(resource.id));
  }
  function toggle(id: string) { setSelected(old => { const next = new Set(old); next.has(id) ? next.delete(id) : next.add(id); return next; }); }

  const title = view==='search'?t('搜索结果：','Results: ')+query : view === 'home' ? t('个人工作台','Personal workbench') : view === 'files' ? mode==='favorites'?t('收藏','Favorites'):t('所有文件','All files') : view === 'workspaces' ? t('每件工作，都有自己的位置。', 'A place for every piece of work.') : view === 'workspace' ? currentWorkspace?.name ?? t('工作区', 'Workspace') : view === 'activity' ? t('回到工作的线索。', 'Find your way back.') : t('由你决定管理范围。', 'You choose what belongs here.');
  const labels: Record<string, string> = { modified: t('最近修改', 'Recently modified'), recent: t('最近使用', 'Recently opened'), favorites: t('收藏', 'Favorites'), files: t('全部资料', 'All resources') };
  const activityLabels: Record<string, string> = { opened: t('应用内打开', 'Opened in app'), linked: t('已关联', 'Linked'), unlinked: t('已移除关联', 'Unlinked'), transferred: t('已转移引用', 'Reference transferred'), workspace_created: t('创建工作区', 'Workspace created'), workspace_deleted: t('删除工作区', 'Workspace deleted'), workspace_renamed: t('工作区改名', 'Workspace renamed'), web_saved: t('保存网页', 'Link saved'), relinked: t('重新指向', 'Relinked') };

  const activeRoots = overview?.roots.filter(root => root.active) ?? [];
  const scanning = activeRoots.some(root => root.location_status !== 'unavailable' && root.scan_state === 'running');
  const fileGrantCount = overview?.file_grants.length ?? 0;
  const hasLocations = activeRoots.length > 0 || fileGrantCount > 0;
  // 尚未读到授权范围时保持中性状态，不能提前判断为首次空库。
  const locationsReady = overview !== null;
  // 首次读取已结束却仍无范围才是读取失败；普通操作错误不改变已就绪的资料状态。
  const initialLoadFailed = native && !locationsReady && !loading;
  // 单文件授权不等于目录授权；分别计数，失联文件仍保留其授权事实。
  const managedSummary = t('已管理 ', 'Managing ') + [
    activeRoots.length ? t(`${activeRoots.length} 个目录`, `${activeRoots.length} folder${activeRoots.length === 1 ? '' : 's'}`) : '',
    fileGrantCount ? t(`${fileGrantCount} 个单独授权文件`, `${fileGrantCount} individually authorized file${fileGrantCount === 1 ? '' : 's'}`) : '',
  ].filter(Boolean).join(' · ');
  const scanLabel: Record<string, string> = {
    idle: t('等待扫描', 'Ready to scan'), running: t('正在扫描', 'Scanning'), completed: t('已完成扫描', 'Scan complete'),
    partial_failure: t('部分位置不可访问', 'Some items unavailable'), cancelled: t('已停止', 'Stopped'), paused: t('已暂停', 'Paused'),
  };
  const rootStatus = (root:Overview['roots'][number]) => {
    if(!root.active)return t('已撤销授权，关联保留','Authorization revoked; references retained');
    // 定位失效不能被旧扫描结果或暂停状态掩盖；授权意图仍由 active 独立表示。
    if(root.location_status==='legacy')return t('请重新选择管理位置以启用目录定位','Select this managed location again to enable folder tracking.');
    if(root.location_status==='unavailable')return t('目录暂不可用，请重新选择管理位置','Folder unavailable. Select the managed location again.');
    if(root.paused)return t('已暂停扫描，已有授权保留','Scanning paused; authorization retained');
    return (scanLabel[root.scan_state]??t('状态待核对','Status pending'))+` · ${root.scanned} `+t('个文件','files')+(root.failures?` · ${root.failures} `+t('项不可访问','unavailable'):'');
  };
  return <div className="app-shell">
    <aside className="sidebar"><button className="brand" onClick={() => navigate('home')}><span><Layers3 size={23} /></span>WorkTwill</button><p className="brand-caption">{t('你的个人工作台', 'Your personal workbench')}</p>
      <nav aria-label={t('主导航','Main navigation')}>{([['home', Home, t('个人工作台', 'Personal workbench')], ['workspaces', Layers3, t('工作区', 'Workspaces')], ['activity', Clock3, t('动态', 'Activity')], ['files', Folder, t('所有文件', 'All files')]] as const).map(([key, Icon, name]) => {
        const current=(view===key&&!(key==='files'&&mode==='favorites'))||(key==='workspaces'&&view==='workspace');
        return <button key={key} className={current?'nav-item active':'nav-item'} aria-label={name} title={name} aria-current={current?'page':undefined} onClick={() => navigate(key)}><Icon size={18} /><span className="nav-label">{name}</span>{key === 'workspaces' && <small>{overview?.workspaces.length ?? 0}</small>}</button>;
      })}<button className={view==='files'&&mode==='favorites'?'nav-item active':'nav-item'} aria-label={t('收藏','Favorites')} title={t('收藏','Favorites')} aria-current={view==='files'&&mode==='favorites'?'page':undefined} onClick={()=>{navigate('files');setMode('favorites');}}><Star size={18}/><span className="nav-label">{t('收藏','Favorites')}</span></button></nav>
      <div className="sidebar-section"><span>{t('我的工作区', 'MY WORKSPACES')}</span><button className="icon-button" title={t('新建工作区', 'New workspace')} onClick={() => setForm({ kind: 'workspace' })} disabled={!native}><Plus size={16} /></button></div>
      <div className="workspace-links">{locationsReady&&!overview.workspaces.length && <p className="sidebar-empty">{t('按正在处理的事情组织资料', 'Organize resources around your work')}</p>}{overview?.workspaces.map((item, index) => <button key={item.id} data-workspace-id={item.id} onClick={() => navigate('workspace', item.id)} className={workspace === item.id ? 'current' : ''}><i className={`dot tone-${index % 4}`} /><span>{item.name}</span><small>{item.count}</small></button>)}</div>

      <div className="sidebar-bottom"><button className={view==='settings'?'nav-item active':'nav-item'} aria-current={view==='settings'?'page':undefined} aria-label={t('管理位置与设置','Locations & settings')} onClick={()=>navigate('settings')}><Settings2 size={18}/><span>{t('管理位置与设置','Locations & settings')}</span></button></div>
    </aside>
    <div className="main-shell"><header className="topbar"><span className="breadcrumb">{t('个人工作台', 'Personal workbench')}<span>/</span><strong>{view === 'workspace' ? currentWorkspace?.name : view === 'home' ? t('概览', 'Overview') : view === 'files' ? mode==='favorites'?t('收藏','Favorites'):t('所有文件','All files') : view === 'workspaces' ? t('工作区', 'Workspaces') : view === 'activity' ? t('动态', 'Activity') : view==='search'?t('搜索结果','Search results'):t('设置', 'Settings')}</strong></span>
      <div className="search-area" ref={searchArea}><div className="search-field"><Search size={17} /><input ref={searchInput} value={query} placeholder={t('搜索文件、网页或工作区', 'Search files, links or workspaces')} aria-label={t('全局搜索', 'Global search')} role="combobox" aria-expanded={searchOpen} aria-controls={searchOpen ? "search-results" : undefined} aria-activedescendant={searchOpen && displayedActive >= 0 ? `search-result-${displayedActive}` : undefined} autoComplete="off" disabled={!native} onFocus={() => setSearchOpen(true)} onChange={event => { setQuery(event.target.value); if(view==='search'){setOffset(0);setPage(null);setSelected(new Set());} setSearchOpen(true); }} onKeyDown={event => {
        if (event.nativeEvent.isComposing) return;
        if (results.length&&(event.key === 'ArrowDown' || event.key === 'ArrowUp')) { event.preventDefault(); setActive(value => value < 0 && defaultActive < 0 ? event.key === 'ArrowDown' ? 0 : results.length - 1 : ((value < 0 ? defaultActive : value) + (event.key === 'ArrowDown' ? 1 : -1) + results.length) % results.length); }
        if (event.key === 'Enter') { event.preventDefault(); if (displayedActive >= 0 && results[displayedActive]) void activate(results[displayedActive]); }
      }} /><kbd>{navigator.platform.includes('Mac') ? '⌘' : 'Ctrl'} K</kbd></div>
      {searchOpen && <SearchPanel onAllResults={addIntent?undefined:showSearchResults} t={t} query={query} searching={searching} results={results} active={displayedActive} scope={searchScope} workspace={workspace} addIntent={addIntent} workspaceName={currentWorkspace?.name} scanning={scanning} hasLocations={hasLocations} onScope={scope=>{setSearchScope(scope);if(view==='search'){setOffset(0);setPage(null);setSelected(new Set());}}} onActive={setActive} onActivate={resource => void activate(resource)} />}

      </div><button className="language-button" onClick={() => setLang(lang === 'zh' ? 'en' : 'zh')}>{lang === 'zh' ? 'EN' : '中文'}</button></header>

      <main ref={mainArea} style={loading&&page===null&&reloadHeight!==undefined?{minHeight:reloadHeight}:undefined} className={view==='activity'?'timeline-main':undefined} data-workspace-id={view==='workspace'?workspace??undefined:undefined}>{view!=='activity'&&<div className="page-title"><div><p className="eyebrow">{view === 'home' ? t('个人工作台', 'YOUR WORK, CONNECTED') : view === 'workspace' ? t('我的工作区', 'WORKSPACE') : 'WORKTWILL'}</p><h1>{title}</h1><p className="subtitle">{view === 'home' ? t('找回最近用过的资料，继续手头的工作。', 'Find recent resources and continue your work.') : view === 'workspace' ? t('关联资料，不改变原文件的位置。', 'Connect resources without moving the originals.') : view === 'settings' ? t('仅索引你明确选择的位置，原件保持只读。', 'Only explicitly selected locations are indexed. Originals stay read-only.') : ''}</p></div><div className="actions">{view !== 'settings' && <><button className="button" disabled={!native || busy} onClick={() => setForm({ kind: 'web' })}><Link2 size={15} />{t('添加网页', 'Add link')}</button><button className={view==='home'&&locationsReady&&!hasLocations?'button':'button primary'} disabled={!native || busy} onClick={() => setForm({ kind: 'workspace' })}><Plus size={16} />{t('新建工作区', 'New workspace')}</button>{view==='home'&&locationsReady&&!hasLocations&&<button className="button primary" disabled={!native||busy} onClick={()=>void run(()=>api.pickRoot(lang))}><Folder size={16}/>{t('添加管理位置','Add location')}</button>}</>}</div></div>}
      {!native && <div className="native-required"><ShieldCheck size={25} /><div><strong>{t('当前为桌面界面预览', 'Desktop interface preview')}</strong><p>{t('原生目录授权、搜索与打开需要通过 pnpm desktop 启动。此页面不加载模拟文件。', 'Run pnpm desktop for native authorization, search and opening. No mock files are loaded here.')}</p></div></div>}
      {error !== null && <div className="error-banner" role="alert"><span>{errorMessage(error.cause,lang)}</span><button className="icon-button" aria-label={t('关闭', 'Dismiss')} onClick={() => setError(null)}><X size={16} /></button></div>}

      {(view === 'home'||view === 'files') && <div className="catalog-status" aria-busy={!locationsReady&&loading}><span className={scanning ? 'status-dot scanning' : 'status-dot'} /><span>{initialLoadFailed ? t('读取失败', 'Loading failed') : !locationsReady ? t('正在读取资料', 'Loading resources') : scanning ? t('正在索引 · ', 'Indexing · ') + managedSummary : hasLocations ? managedSummary : t('尚未添加管理位置', 'No managed locations yet')}</span><button className="text-button" onClick={() => navigate('settings')}>{t('管理范围', 'Manage locations')}<ChevronRight size={14} /></button></div>}
      {(view === 'workspaces'||(view === 'home'&&(hasLocations||!!overview?.workspaces.length))) && <section className="workspace-section" aria-label={view==='home'?t('我的工作区','My workspaces'):t('工作区列表','Workspace list')}><div className="section-heading"><h2>{view==='home'?t('我的工作区','My workspaces'):t('从工作区继续', 'Continue in a workspace')}</h2>{view==='home'?<button className="text-button" onClick={()=>navigate('workspaces')}>{t('查看全部工作区','View all workspaces')}<ChevronRight size={14}/></button>:<span>{overview?.workspaces.length ?? 0} {t('个工作区', 'workspaces')}</span>}</div><div className="workspace-grid">{(view==='home'?overview?.workspaces.slice(0,3):overview?.workspaces)?.map((item, index) => <button key={item.id} data-workspace-id={item.id} className={`workspace-card tone-${index % 4}`} onClick={() => navigate('workspace', item.id)}><span className="card-icon"><Folder size={23} /></span><ArrowRight size={17} className="card-arrow" /><h3>{item.name}</h3><p>{item.count} {t('份资料', 'resources')}</p></button>)}{(view==='workspaces'||!overview?.workspaces.length)&&<button className="workspace-card create-card" disabled={!native} onClick={() => setForm({ kind: 'workspace' })}><Plus size={24} /><span>{t('为一件工作，留一个位置', 'Make space for your next project')}</span></button>}</div></section>}

      {(view==='files'||view==='workspace')&&!!overview?.workspaces.length&&<div className="workspace-drop-tray"><span>{t('拖放加入工作区','Drop to add to a workspace')}</span><div>{overview.workspaces.map(item=><button data-workspace-id={item.id} key={item.id} onClick={()=>navigate('workspace',item.id)}><Folder size={15}/>{item.name}<small>{item.count}</small></button>)}</div></div>}
      {view === 'workspace' && <div className="workspace-toolbar"><div className="filter-tabs">{[['all', t('全部', 'All')], ['file', t('文件', 'Files')], ['web', t('网页', 'Links')]].map(([value, text]) => <button key={value} className={kind === value ? 'on' : ''} onClick={() => { setKind(value); setOffset(0); setSelected(new Set()); }}>{text}</button>)}</div><div className="actions"><button className="button" onClick={() => setForm({ kind: 'rename', id: workspace!, name: currentWorkspace?.name })}>{t('改名', 'Rename')}</button><button className="button subtle-danger" disabled={busy} onClick={() => setForm({ kind: 'delete', id: workspace!, name: currentWorkspace?.name })}><Trash2 size={15} />{t('删除工作区', 'Delete workspace')}</button><button className="button primary" onClick={() => { setAddIntent(true); setSearchScope('all'); setSearchOpen(true); searchInput.current?.focus(); }}><Plus size={15} />{t('添加文件', 'Add files')}</button><button className="button" disabled={busy||!native} onClick={()=>void run(()=>api.pickFiles(workspace,lang)).then(result=>{if(result)setDropResult({token:'',workspace,result});})}>{t('选择本地文件','Choose local files')}</button></div></div>}

      {view==='search'&&searchOrigin&&<div className="search-return-bar"><button className="button" onClick={returnFromSearch}><ArrowLeft size={15}/><span>{returnLabel}</span></button></div>}
      {view==='search'&&<div className="workspace-toolbar"><div className="filter-tabs">{[['all',t('全部','All')],['file',t('文件','Files')],['web',t('网页','Links')],['workspace',t('工作区','Workspaces')]].map(([value,label])=><button key={value} className={kind===value?'on':''} onClick={()=>{setKind(value);setOffset(0);setPage(null);setSelected(new Set());}}>{label}</button>)}</div><select aria-label={t('搜索排序','Search sort')} value={sort} onChange={event=>{setSort(event.target.value);setOffset(0);setPage(null);setSelected(new Set());}}><option value="relevance">{t('相关性','Relevance')}</option><option value="name">{t('名称','Name')}</option><option value="modified">{t('最近修改','Recently modified')}</option></select></div>}
      {view==='search'&&<><p className="search-page-scope">{searchScope==='current'?t('当前工作区：','Workspace: ')+(currentWorkspace?.name??''):t('全部资料','All resources')}</p><SearchFilters t={t} value={searchFilters} onChange={value=>{setSearchFilters(value);setOffset(0);setPage(null);setSelected(new Set());}}/></>}
      {(view === 'home' || view === 'files' || view === 'workspace' || view==='search') && <section className="resources-section">{(view === 'home'||view === 'files') && <div className="section-heading"><div className="filter-tabs">{Object.entries(labels).map(([value, text]) => <button className={mode === value ? 'on' : ''} key={value} onClick={() => { setMode(value); setOffset(0); setSelected(new Set()); }}>{text}</button>)}</div><button className="text-button" disabled={!native} onClick={() => void run(() => api.rescan())}><RefreshCw size={14} />{t('重新扫描', 'Rescan')}</button></div>}
        {selected.size > 0 && <div className="selection-bar"><strong>{selected.size} {t('项已选择', 'selected')}</strong><button onClick={() => setForm({ kind: 'link', resourceIds: [...selected] })}>{t('加入工作区', 'Add to workspace')}</button><button disabled={busy} onClick={()=>setForm({kind:'workspace',resourceIds:[...selected]})}>{t('新建工作区并加入','Create workspace with selection')}</button>{view === 'workspace' && <><button onClick={() => setForm({ kind: 'transfer', resourceIds: [...selected] })}>{t('移到工作区', 'Move references')}</button><button onClick={() => void mutate({ type: 'unlink', workspace_id: workspace!, resource_ids: [...selected] }, t('已移除关联，原件保留', 'References removed. Originals retained.'))}>{t('从本工作区移除', 'Remove from this workspace')}</button></>}<button className="clear-selection" aria-label={t('取消选择','Clear selection')} onClick={() => setSelected(new Set())}><X size={16} /></button></div>}
        {page?.items.length ? <ResourceList onDragStart={(event,ids)=>{if(!busy)startResourceDrag(event,ids);}} t={t} lang={lang} items={page.items} selected={selected} busy={busy} onToggle={toggle} onOpen={resource => void activate(resource)} onFavorite={resource => void mutate({ type: 'favorite', id: resource.id, value: !resource.favorite })} onReveal={resource => void run(() => api.reveal(resource.id))} onRelink={resource => void run(() => api.relink(resource.id,lang))} onDeleteWeb={resource=>setForm({kind:'deleteweb',id:resource.id,name:resource.name})} onEdit={resource => setForm({ kind: 'web', id: resource.id, name: resource.name, url: resource.location })} /> : initialLoadFailed ? <div className="empty-state"><h3>{t('读取失败', 'Loading failed')}</h3><button className="button" disabled={busy} onClick={()=>{setError(null);setLoading(true);reload();}}>{t('重新加载', 'Try again')}</button></div> : <EmptyState t={t} loading={loading && native} view={view} mode={mode} kind={kind} hasLocations={hasLocations} disabled={!native || busy} onLocation={suggestion => void run(() => api.pickRoot(lang,suggestion))} onAdd={() => { setAddIntent(true); setSearchScope('all'); setSearchOpen(true); searchInput.current?.focus(); }} onWeb={() => setForm({ kind: 'web' })} />}

        {page?.has_more && <button className="button load-more" disabled={loading} onClick={() => {if(view==='search'){if(nextSearchCursor.current)setOffset(nextSearchCursor.current.next_offset);}else setOffset(value=>value+100);}}>{t('加载更多', 'Load more')}</button>}
      </section>}

      {view==='activity'&&native&&overview&&!hasLocations&&<section className="first-location-banner"><div><strong>{t('从一个文件夹开始，找回工作的线索。','Start with a folder. Find your way back.')}</strong><p>{t('选择要管理的位置，文件原地不动。','Choose a location to manage. Your files stay in place.')}</p><LocationSuggestions t={t} disabled={busy} onSelect={suggestion=>void run(()=>api.pickRoot(lang,suggestion))}/></div><button className="button primary" disabled={busy} onClick={()=>void run(()=>api.pickRoot(lang))}><Plus size={16}/>{t('添加管理位置','Add location')}</button></section>}
      {view === 'activity' && (native ? <ActivityTimeline lang={lang} workspacesReady={!!overview} revision={refresh} workspaces={overview?.workspaces??[]} onOpen={resource=>void run(()=>api.open(resource.id))} onReveal={resource=>void run(()=>api.reveal(resource.id))} onWorkspace={id=>navigate('workspace',id)} /> : <div className="empty-state"><Clock3 size={32}/><h2>{t('活动时间线','Activity timeline')}</h2><p>{t('在桌面应用中添加管理位置后，可按日历查看文件修改与首次发现记录。','Add managed locations in the desktop app to browse file changes by date.')}</p></div>)}

      {view === 'settings' && <section className="settings"><div className="section-heading"><h2>{t('管理位置', 'Managed locations')}</h2><button className="button primary" disabled={!native || busy} onClick={() => void run(() => api.pickRoot(lang))}><Plus size={15} />{t('添加管理位置', 'Add location')}</button></div>{overview&&!hasLocations&&<div className="suggested-location-card"><LocationSuggestions t={t} disabled={!native||busy} onSelect={suggestion=>void run(()=>api.pickRoot(lang,suggestion))}/></div>}{overview?.roots.map(root => <div className="location-card" key={root.id}><div className="location-main"><Folder size={23} /><div className="location-copy"><strong>{root.path}</strong><small>{rootStatus(root)}</small></div><div className="actions">{root.active && <><button className="button" disabled={busy} onClick={() => void mutate({ type: 'set_paused', id: root.id, value: !root.paused })}>{root.paused ? t('继续扫描', 'Resume') : t('暂停扫描', 'Pause')}</button><button className="button danger" disabled={busy} onClick={() => void mutate({ type: 'revoke_root', id: root.id })}>{t('撤销授权', 'Revoke')}</button></>}</div></div>{root.exclusions.length > 0 && <p className="exclusions">{t('已排除：', 'Excluded: ')}{root.exclusions.map(path=><button type="button" className="exclusion-chip" key={path} disabled={busy||!root.active} title={t('恢复索引此子目录','Include this subfolder')} onClick={()=>void mutate({type:'include',root_id:root.id,relative_path:path},t('已恢复此子目录的索引','Subfolder included'))}>{path}<X size={12}/></button>)}</p>}{root.active && <form className="exclude-form" onSubmit={event => { event.preventDefault(); const value=excludedFolders[root.id]??''; void mutate({ type: 'exclude', root_id: root.id, relative_path: value }).then(result=>{if(result)setExcludedFolders(current=>current[root.id]===value?{...current,[root.id]:''}:current);}); }}><input name="relative" disabled={busy} aria-label={t('排除子目录：','Exclude subfolder in ')+root.path} value={excludedFolders[root.id]??''} onChange={event=>setExcludedFolders(current=>({...current,[root.id]:event.target.value}))} placeholder={t('排除相对子目录，例如 archive/private', 'Exclude subfolder, e.g. archive/private')} required /><button className="button" disabled={busy}>{t('排除子目录', 'Exclude folder')}</button></form>}</div>)}
        {!!overview?.file_grants.length&&<><div className="settings-divider"/><h2>{t('单独授权的文件','Individually authorized files')}</h2><p className="muted">{t('仅访问选中的文件，不扫描所在文件夹。','Only the selected files are managed; their folders are not scanned.')}</p>{overview.file_grants.map(file=><div className="location-card" key={file.id}><div className="location-main"><div className="location-copy"><strong>{file.name}</strong><small>{file.location}</small>{file.status!=='available'&&<small>{file.status==='identity_unconfirmed'?t('身份待确认','Identity unconfirmed'):t('原件暂不可用','Original unavailable')}</small>}</div><button className="button danger" disabled={busy} onClick={()=>void mutate({type:'revoke_file',id:file.id})}>{t('撤销授权','Revoke')}</button></div></div>)}</>}<div className="settings-divider"/><h2>{t('工作区顺序','Workspace order')}</h2><div className="workspace-order">{overview?.workspaces.map((item,index)=><div key={item.id}><Folder size={17}/><span>{item.name}</span>{[-1,1].map(direction=><button key={direction} className="button" disabled={busy||index+direction<0||index+direction>=overview.workspaces.length} aria-label={(direction<0?t('上移 ','Move up '):t('下移 ','Move down '))+item.name} onClick={()=>{const ids=overview.workspaces.map(w=>w.id);[ids[index],ids[index+direction]]=[ids[index+direction],ids[index]];void mutate({type:'reorder_workspaces',ids});}}>{direction<0?'↑':'↓'}</button>)}</div>)}</div><div className="settings-divider" /><h2>{t('本地数据', 'Local data')}</h2><p className="muted">{t('备份保存工作区、关联与 URL，不包含原文件；可能含敏感名称和路径。', 'Backups contain workspaces, references and URLs, not original files. Names and paths may be sensitive.')}</p><div className="actions"><button className="button" disabled={!native || busy} onClick={() => void run(() => api.backup(lang)).then(done => { if (done) setNotice(t('备份已保存', 'Backup saved')); })}>{t('备份数据', 'Back up')}</button><button className="button" disabled={!native || busy} onClick={() => void run(() => api.restore(lang)).then(done => { if (done) setNotice(t('已恢复，请重新选择管理位置', 'Restored. Please reauthorize locations.')); })}>{t('恢复备份', 'Restore backup')}</button><button className="button" disabled={!native || busy} onClick={() => void mutate({ type: 'rebuild_search' }, t('搜索索引已重建', 'Search index rebuilt'))}>{t('重建搜索索引', 'Rebuild search index')}</button><button className="button" disabled={!native || busy} onClick={() => void run(() => api.rescan())}>{t('重新扫描', 'Rescan')}</button></div><p className="technical-note">WorkTwill 0.1.0 · SQLite {overview?.sqlite_version ?? '—'} · {t('原生开发版本，保存追踪范围待验证', 'Native development build. Save-tracking support under validation.')}</p>
      </section>}
      <footer><ShieldCheck size={13} />{t('原件在原处，工作在一起。', 'Originals in place. Your work together.')}</footer>
      </main></div>

    {form && <FormDialog key={form.kind + (form.id ?? '')} form={form} lang={lang} busy={busy} error={error} workspaces={overview?.workspaces ?? []} source={view==='workspace'?workspace:null} onClose={() => setForm(null)} onSave={saveForm} />}
    {batchReport&&<div className="batch-report" role="status"><div><strong>{batchReport.succeeded} {t('项成功','succeeded')} · {batchReport.failures.length} {t('项失败','failed')}</strong>{!!batchReport.failures.length&&<p>{t('失败项保留原有引用。','Failed items retain their original references.')}</p>}<ul>{batchReport.failures.map((item,index)=><li key={index}><b>{item.name??t('不可用资料','Unavailable resource')}</b> · {errorMessage(item.cause,lang)}</li>)}</ul></div><button className="button" onClick={()=>setBatchReport(null)}>{t('完成','Done')}</button></div>}
    {dropPreparing&&<div className="external-drag-hint" role="status"><LoaderCircle className="spin" size={18}/>{t('正在确认拖入的文件…','Checking dropped files…')}<button className="text-button" onClick={()=>{const pending=startedDrop.current;startedDrop.current=null;dropSequence.current++;setDropPreparing(false);if(pending)void api.cancelDrop(pending.token).catch(cause=>setError({cause}));}}>{t('取消拖入','Cancel drop')}</button></div>}
    {externalDrag&&<div className="external-drag-hint" role="status"><Plus size={18}/>{t('松开添加文件 · 拖到工作区可直接关联','Drop to add files · Drop on a workspace to link them')}</div>}
    {drag && <div className="drag-preview" style={{left:drag.x+14,top:drag.y+14}}>{drag.count} {t('份资料 · 拖到工作区加入','resources · drop on a workspace')}</div>}
    {dropResult && <div className="import-summary" role="status"><div><strong>{t('文件添加结果','File import results')}</strong><p>{dropResult.result.items.filter(i=>i.status==='added').length} {t('项已加入','added')} · {dropResult.result.items.filter(i=>i.status==='already_present').length} {t('项已存在','already present')} · {dropResult.result.items.filter(i=>i.status==='not_authorized').length} {t('项未授权','need authorization')} · {dropResult.result.items.filter(i=>i.status==='failed').length} {t('项失败','failed')}</p>{dropResult.result.items.filter(i=>i.status==='failed').slice(0,3).map(i=><small key={i.path}>{i.path.split(/[\\/]/).pop()} · {errorMessage(i,lang)}</small>)}</div><div className="actions">{dropResult.token&&dropResult.result.items.some(i=>i.status==='not_authorized')&&<button className="button primary" disabled={busy} onClick={()=>void run(()=>api.importDrop(dropResult.token,dropResult.workspace,true,lang)).then(result=>{if(result)setDropResult(current=>current?.token===dropResult.token?{...current,result:{items:[...current.result.items.filter(i=>i.status!=='not_authorized'),...result.items]}}:current);})}>{t('仅授权未加入的文件','Authorize remaining files')}</button>}<button className="button" onClick={()=>{const token=dropResult.token;setDropResult(null);if(token)void api.cancelDrop(token).catch(cause=>setError({cause}));}}>{t('完成','Done')}</button></div></div>}
    {notice && <div className="toast" role="status"><Check size={17} />{notice}</div>}
  </div>;
}

function FormDialog({ form, lang, busy, error, workspaces, source, onClose, onSave }: { form: FormState; lang: Lang; busy: boolean; error: Failure; workspaces: Overview['workspaces']; source: string | null; onClose: () => void; onSave: (mutation: Mutation) => Promise<void> }) {
  const t = (zh: string, en: string) => lang === 'zh' ? zh : en;
  const dialog = useRef<HTMLDialogElement>(null);
  const [name, setName] = useState(form.name ?? '');
  const [url, setUrl] = useState(form.url ?? '');
  const [fetchTitle,setFetchTitle]=useState(!form.id);
  const [metadataState,setMetadataState]=useState<'idle'|'loading'|'ready'|'failed'>('idle');
  const [metadataError,setMetadataError]=useState<Failure>(null);
  const editedTitle=useRef(!!form.id);
  const metadataSequence=useRef(0);
  useEffect(()=>{
    const sequence=++metadataSequence.current;
    if(form.kind!=='web'){setMetadataState('idle');return;}
    let parsed:URL;
    try{parsed=new URL(webUrl(url));if(!['http:','https:'].includes(parsed.protocol)||!parsed.hostname){setMetadataState('idle');return;}}catch{setMetadataState('idle');return;}
    if(!editedTitle.current)setName(parsed.hostname.replace(/^www\./,''));
    if(!fetchTitle||!native){setMetadataState('idle');return;}
    setMetadataState('loading');setMetadataError(null);
    const timer=window.setTimeout(()=>{
      void api.webMetadata(webUrl(url)).then(data=>{
        if(sequence!==metadataSequence.current)return;
        if(data.title){if(!editedTitle.current)setName(data.title);setMetadataState('ready');}
        else{setMetadataState('failed');setMetadataError({cause:{message:'网站未提供标题，可自行修改名称',message_en:'No title provided. You can edit the name.'}});}
      }).catch(cause=>{if(sequence===metadataSequence.current){setMetadataState('failed');setMetadataError({cause});}});
    },600);
    return()=>{window.clearTimeout(timer);metadataSequence.current++;};
  },[url,fetchTitle,form.kind]);
  const [target, setTarget] = useState('');
  const linking = form.kind === 'link' || form.kind === 'transfer';
  useEffect(() => { dialog.current?.showModal(); if(form.kind!=='delete'&&form.kind!=='deleteweb') dialog.current?.querySelector<HTMLInputElement>('input[type="url"],input:not([type="checkbox"]),select')?.focus(); }, []);
  return <dialog ref={dialog} onCancel={onClose} className="modal" aria-labelledby="dialog-title"><form onSubmit={event => { event.preventDefault(); let mutation: Mutation;
    if (form.kind === 'workspace') mutation = { type: 'create_workspace', name, ...(form.resourceIds?.length?{resource_ids:form.resourceIds}:{}) };
    else if (form.kind === 'delete') mutation = { type: 'delete_workspace', id: form.id! };
    else if (form.kind === 'deleteweb') mutation = { type: 'delete_web', id: form.id! };
    else if (form.kind === 'rename') mutation = { type: 'rename_workspace', id: form.id!, name };
    else if (form.kind === 'web') mutation = { type: 'save_web', id: form.id ?? null, name, url:webUrl(url), workspace_id: form.id ? null : source };
    else if (form.kind === 'transfer') mutation = { type: 'transfer', source: source!, target, resource_ids: form.resourceIds! };
    else mutation = { type: 'link', workspace_id: target, resource_ids: form.resourceIds! };
    void onSave(mutation);
  }}><div className="modal-heading"><h2 id="dialog-title">{form.kind === 'delete' ? t('删除工作区', 'Delete workspace') : form.kind === 'deleteweb' ? t('删除网页', 'Delete link') : form.kind === 'web' ? t('保存网页', 'Save web link') : form.kind === 'rename' ? t('工作区改名', 'Rename workspace') : form.kind === 'transfer' ? t('移到工作区', 'Move references') : linking ? t('加入工作区', 'Add to workspace') : form.resourceIds?.length?t('新建工作区并加入','Create workspace with selection'):t('新建工作区', 'New workspace')}</h2><button type="button" className="icon-button" aria-label={t('关闭', 'Close')} onClick={onClose}><X size={19} /></button></div>
    <div className="modal-body">{form.kind==='workspace'&&!!form.resourceIds?.length&&<p className="muted">{t(`将 ${form.resourceIds.length} 项资料加入新工作区`,`Add ${form.resourceIds.length} selected resources to the new workspace`)}</p>}{error !== null && <p role="alert" className="form-error">{errorMessage(error.cause,lang)}</p>}{form.kind === 'delete' ? <p className="muted">{t('删除「', 'Delete “') + form.name + t('」？仅移除这个工作区与其中的关联，原件及其他工作区保留。', '”? Only this workspace and its references are removed. Originals and other workspaces remain.')}</p> : form.kind === 'deleteweb' ? <p className="muted">{t('删除「','Delete “')+form.name+t('」？将移除此网页在所有工作区、收藏和动态中的记录。','”? This removes the link from all workspaces, favorites and activity.')}</p> : linking ? <><p className="muted">{t('只修改应用中的引用，原件原地不动。', 'Only app references change. Original files stay in place.')}</p><label>{t('目标工作区', 'Target workspace')}<select required value={target} onChange={event => setTarget(event.target.value)} autoFocus><option value="">{t('请选择', 'Select workspace')}</option>{workspaces.filter(w => form.kind !== 'transfer' || w.id !== source).map(w => <option key={w.id} value={w.id}>{w.name}</option>)}</select></label></> : <>{form.kind === 'web' && <><label>{t('网址', 'URL')}<input required type="text" inputMode="url" value={url} onBlur={()=>setUrl(webUrl(url))} onChange={event => setUrl(event.target.value)} placeholder="https://" autoFocus /></label><label className="metadata-toggle"><input type="checkbox" checked={fetchTitle} onChange={event=>setFetchTitle(event.target.checked)}/>{t('自动获取网站标题','Fetch page title automatically')}</label><p className="metadata-privacy">{t('会访问目标网站，并使用 Cloudflare 解析域名；不读取浏览器登录状态。','Visits the website and uses Cloudflare to resolve its domain, without browser login data.')}</p></>}<label>{form.kind==='web'?t('标题，可修改','Title, editable'):t('名称', 'Name')}<input required maxLength={200} value={name} onChange={event => {editedTitle.current=true;setName(event.target.value);}} autoFocus={form.kind!=='web'} placeholder={form.kind==='web'?t('输入网址后自动填写','Filled from the URL'):t('例如：客户 A · 合作项目', 'e.g. Client A · Project')} /></label>{form.kind==='web'&&metadataState!=='idle'&&<p className="metadata-feedback" role="status">{metadataState==='loading'?t('正在获取标题… 可继续编辑或保存','Fetching title… You can keep editing or save.'):metadataState==='ready'?t('已获取网站标题','Page title retrieved'):metadataError!==null?errorMessage(metadataError.cause,lang):''}</p>}</>}</div>
    <div className="modal-footer"><button type="button" className="button" onClick={onClose}>{t('取消', 'Cancel')}</button><button className={(form.kind === 'delete'||form.kind==='deleteweb') ? 'button destructive' : 'button primary'} disabled={busy}>{busy ? <LoaderCircle size={15} className="spin" /> : <Check size={15} />}{form.kind === 'delete' ? t('删除工作区', 'Delete workspace') : form.kind==='deleteweb'?t('删除网页','Delete link'):t('保存', 'Save')}</button></div></form></dialog>;
}

