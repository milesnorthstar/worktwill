import { useEffect, useRef, useState } from 'react';
import { ArrowDownLeft, ChevronRight, Download, File, Folder, FolderOpen, Link2, LoaderCircle, Monitor, MoreHorizontal, X, Plus, Search, Star, Trash2 } from 'lucide-react';
import type { Resource, SuggestedLocation } from '../contracts';

export type Translator = (zh: string, en: string) => string;

export function LocationSuggestions({t,disabled,onSelect}:{t:Translator;disabled:boolean;onSelect:(location:SuggestedLocation)=>void}) {
  return <div className="location-suggestions" role="group" aria-label={t('常用位置','Common locations')}><span>{t('常用位置','Common locations')}</span>{([['desktop',t('桌面','Desktop'),Monitor],['downloads',t('下载','Downloads'),Download],['documents',t('文档','Documents'),File]] as const).map(([location,label,Icon])=><button key={location} className="button" disabled={disabled} onClick={()=>onSelect(location)}><Icon size={15}/>{label}</button>)}</div>;
}

export type SearchFilterValues={from:string;to:string;path:string};
export function SearchFilters({t,value,onChange,onCompositionStart,onCompositionEnd}:{onCompositionStart?:()=>void;onCompositionEnd?:()=>void;t:Translator;value:SearchFilterValues;onChange:(next:SearchFilterValues)=>void}) {
  return <div className="search-filter-bar"><label>{t('修改日期，从','Modified, from')}<input type="date" value={value.from} max={value.to||undefined} onChange={event=>onChange({...value,from:event.target.value})}/></label><label>{t('至','Through')}<input type="date" value={value.to} min={value.from||undefined} onChange={event=>onChange({...value,to:event.target.value})}/></label><label className="path-filter">{t('路径或网址前缀','Path or URL prefix')}<input onCompositionStart={onCompositionStart} onCompositionEnd={onCompositionEnd} value={value.path} placeholder={t('例如 /资料/ 或 https://example.com/','e.g. /Documents/ or https://example.com/')} onChange={event=>onChange({...value,path:event.target.value})}/></label><button className="text-button" disabled={!value.from&&!value.to&&!value.path} onClick={()=>onChange({from:'',to:'',path:''})}>{t('清除筛选','Clear filters')}</button></div>;
}

type SearchProps = {
  kind?:string;onKind?:(kind:string)=>void;onClose?:()=>void;
  t: Translator; query: string; searching: boolean; results: Resource[]; active: number;
  scope: 'all' | 'current'; workspace: string | null; workspaceName?: string;
  addIntent: boolean; scanning: boolean; hasLocations: boolean;
  onScope: (scope: 'all' | 'current') => void; onActive: (index: number) => void;
  onActivate: (resource: Resource) => void; onAllResults?:()=>void;
};

export function SearchPanel({ kind:quickKind='all',onKind,onClose,t, query, searching, results, active, scope, workspace, workspaceName, addIntent, scanning, hasLocations, onScope, onActive, onActivate, onAllResults }: SearchProps) {
  useEffect(() => {
    document.getElementById(`search-result-${active}`)?.scrollIntoView?.({ block: 'nearest' });
  }, [active]);
  const groups = [['workspace', t('工作区', 'Workspaces')], ['file', t('文件', 'Files')], ['web', t('网页', 'Links')]];
  return <div className="search-panel"><div className="quick-sidebar"><h3>{t('资料类型','Resource types')}</h3>{[['all',t('全部结果','All results')],...groups].map(([value,label])=><button key={value} className={quickKind===value?'on':''} onClick={()=>onKind?.(value)}>{label}</button>)}<p>{t('搜索已授权的本地资料','Search authorized local resources')}</p><button className="text-button" onClick={onClose}><X size={14}/>{t('关闭搜索','Close search')}</button></div><div className="quick-content">
    <div className="search-scope"><span>{addIntent ? t('添加到：', 'Add to: ') + workspaceName : t('搜索范围', 'Search scope')}</span>
      <button className={scope === 'all' ? 'chip on' : 'chip'} onClick={() => onScope('all')}>{addIntent ? t('全部已授权文件', 'All authorized files') : t('全部资料', 'All resources')}</button>
      {workspace && !addIntent && <button className={scope === 'current' ? 'chip on' : 'chip'} onClick={() => onScope('current')}>{t('当前工作区', 'This workspace')}</button>}
    </div>
    {!searching&&groups.filter(([kind])=>results.some(item=>item.kind===kind)).length>1&&<div className="search-group-jumps" aria-label={t('结果分类','Result groups')}>{groups.map(([kind,label])=>{const count=results.filter(item=>item.kind===kind).length;return count?<button key={kind} aria-label={t('跳到','Jump to ')+label+t('结果',' results')} onMouseDown={event=>event.preventDefault()} onClick={()=>{onActive(results.findIndex(item=>item.kind===kind));document.getElementById(`search-group-${kind}`)?.scrollIntoView?.({block:'start'});}}>{label}<small>{count}</small></button>:null;})}</div>}
    <div id="search-results" role="listbox" aria-label={t('搜索结果', 'Search results')} aria-busy={searching} className="search-results">
      {!query.trim() ? <div className="search-empty"><Search size={24} /><strong>{t('输入名称或路径', 'Search by name or path')}</strong><span>{hasLocations ? t('查找已授权文件、已存网页与工作区', 'Find authorized files, saved links and workspaces') : t('可搜索已存网页与工作区；查找文件请先添加管理位置', 'Search saved links and workspaces. Add a location to find files.')}</span></div>
        : searching ? <div className="search-empty" role="status"><LoaderCircle size={22} className="spin" />{t('正在搜索…', 'Searching…')}</div>
          : !results.length ? <div className="search-empty"><Search size={24} /><strong>{t('没有匹配资料', 'No matching resources')}</strong><span>{t('试试更短的名称，或检查管理范围', 'Try a shorter name or check managed locations')}</span></div>
            : groups.map(([kind, label]) => {
              const items = results.filter(item => item.kind === kind);
              return items.length ? <div id={`search-group-${kind}`} role="group" aria-label={label} key={kind} className="search-group"><div className="search-group-heading"><span>{label}</span><small>{items.length}</small></div>
                {items.map(item => {
                  const index = results.indexOf(item);
                  return <button key={item.id} id={`search-result-${index}`} role="option" disabled={searching} aria-selected={active === index} className={`search-result ${active === index ? 'highlighted' : ''}`} onMouseEnter={() => onActive(index)} onClick={() => onActivate(item)}>
                    <span className={`resource-icon ${item.kind}`}>{item.kind === 'workspace' ? <Folder size={18} /> : item.kind === 'web' ? <Link2 size={18} /> : <File size={18} />}</span>
                    <span><strong>{item.name}</strong>{item.match_field&&<em className="match-field">{item.match_field==='name'?t('名称匹配','Name match'):item.match_field==='url'?t('网址匹配','URL match'):t('路径匹配','Path match')}</em>}<small title={item.location}>{item.location || t('工作区', 'Workspace')}</small></span>
                    <span className="search-result-action">{addIntent ? t('加入', 'Add') : t('打开', 'Open')}<ArrowDownLeft size={14} /></span>
                  </button>;
                })}</div> : null;
            })}
    </div>
    {onAllResults&&<button className="search-all-results" onClick={onAllResults}>{t("查看全部搜索结果","View all results")}<ChevronRight size={15}/></button>}
    <div className="search-footer"><span><kbd>↑</kbd><kbd>↓</kbd>{t('选择', 'Select')} <kbd>↵</kbd>{addIntent ? t('加入', 'Add') : t('打开', 'Open')} <kbd>Esc</kbd>{t('返回', 'Close')}</span><span>{scanning ? t('索引中', 'Indexing') : t('已授权范围', 'Authorized scope')}</span></div>
  </div></div>;
}

type ResourceListProps = {
  resetKey?:string;
  timeMode?:'opened'|'modified';
  t: Translator; lang: 'zh' | 'en'; items: Resource[]; selected: Set<string>; busy: boolean;
  onToggle: (id: string) => void; onOpen: (item: Resource) => void;
  onFavorite: (item: Resource) => void; onReveal: (item: Resource) => void;
  onDeleteWeb?: (item:Resource)=>void; onRelink: (item: Resource) => void; onEdit: (item: Resource) => void; onDragStart?: (event:React.PointerEvent,ids:string[])=>void;
};

export function ResourceList({ resetKey,timeMode='modified',t, lang, items, selected, busy, onToggle, onOpen, onFavorite, onReveal, onRelink, onEdit, onDeleteWeb, onDragStart }: ResourceListProps) {
  const viewport = useRef<HTMLDivElement>(null);
  const [scrollTop, setScrollTop] = useState(0);
  // 完整首页已达100项时就使用同一滚动区，续页不能因切换布局把末尾位置挤回开头。
  const virtual = items.length >= 100;
  // 固定行高与少量预渲染保持滚动稳定，不让持续加载把所有文件节点堆入 DOM。
  const rowHeight = 84;
  const first = virtual ? Math.max(0, Math.floor(scrollTop / rowHeight) - 4) : 0;
  const last = virtual ? Math.min(items.length, first + 16) : items.length;
  useEffect(() => { setScrollTop(0); if (viewport.current) viewport.current.scrollTop = 0; }, [resetKey??items[0]?.id]);
  return <div className="resource-table">
    <div className="table-heading"><span>{t('资料名称', 'Resource')}</span><span>{timeMode==='opened'?t('最近打开','Opened'):t('最近修改', 'Modified')}</span><span>{t('操作', 'Actions')}</span></div>
    <div ref={viewport} className={virtual ? 'resource-viewport virtual' : 'resource-viewport'} onScroll={event=>setScrollTop(event.currentTarget.scrollTop)} role="region" aria-label={t('资料列表','Resource list')} tabIndex={virtual?0:undefined}>
    {virtual&&<div aria-hidden="true" style={{height:first*rowHeight}}/>}
    {items.slice(first,last).map(item => {
      const available = item.status === 'available';
      const name=item.status==='permission_denied'?t('未授权的文件','Unauthorized file'):item.name;
      const extension = item.kind === 'web' ? 'URL' : item.name.includes('.') ? item.name.split('.').pop()!.slice(0, 8).toUpperCase() : 'FILE';
      const parent = item.kind === 'web' ? item.location : item.location.replace(/[\\/][^\\/]*$/, '');
      const status = item.status === 'permission_denied' ? t('未授权 · 关联已保留', 'Not authorized · reference retained') : item.status === 'identity_unconfirmed' ? t('身份变化，请确认原件', 'Identity changed · confirm original') : t('原件不可用', 'Original unavailable');
      return <div className={`resource-row ${selected.has(item.id) ? 'selected' : ''}`} key={item.id} onPointerDown={event=>{if(item.kind!=='workspace')onDragStart?.(event,selected.has(item.id)?[...selected]:[item.id]);}}>
        <div className="resource-copy"><input type="checkbox" disabled={busy||item.kind==='workspace'} aria-label={t('选择 ', 'Select ') + name} checked={selected.has(item.id)} onChange={() => onToggle(item.id)} />
          <span className={`resource-icon ${item.kind}`}>{item.kind === 'web' ? <Link2 size={20} /> : item.kind==='workspace'?<Folder size={20}/>:<File size={20} />}</span>
          <button className="resource-name" disabled={busy||!available} onKeyDown={event=>{if(event.repeat)event.preventDefault();}} onClick={event=>{if(event.detail<2)onOpen(item);}} title={item.kind==='workspace'?t('打开工作区','Open workspace'):available ? t('打开原件', 'Open original') : status}><strong>{name}</strong><small title={item.location}>{item.status === 'permission_denied' ? t('重新授权后可访问', 'Authorize the location to access') : parent}</small>{!available && <em>{status}</em>}</button>
          <span className="file-type">{extension}</span>
        </div>
        <span className="date">{(timeMode==='opened'?item.opened_at:item.modified_at) ? new Date((timeMode==='opened'?item.opened_at:item.modified_at)!).toLocaleDateString(lang === 'zh' ? 'zh-CN' : 'en-US') : '—'}</span>
        <div className="row-actions">
          {item.kind!=='workspace' && item.status !== 'permission_denied' && <button className={`icon-button ${item.favorite ? 'is-favorite' : ''}`} disabled={busy} title={item.favorite ? t('取消收藏', 'Unfavorite') : t('收藏', 'Favorite')} aria-label={(item.favorite ? t('取消收藏 ', 'Unfavorite ') : t('收藏 ', 'Favorite ')) + name} onClick={() => onFavorite(item)}><Star size={16} fill={item.favorite ? 'currentColor' : 'none'} /></button>}
          {available && item.kind==='workspace' && <button className="icon-button" disabled={busy} title={t('打开', 'Open')} aria-label={t('打开 ', 'Open ') + name} onClick={() => onOpen(item)}><ChevronRight size={18} /></button>}
          {item.kind === 'file' && available && <button className="icon-button" disabled={busy} title={t('显示原始位置', 'Reveal original')} aria-label={t('显示原始位置 ', 'Reveal original ') + name} onClick={() => onReveal(item)}><FolderOpen size={17} /></button>}
          {item.kind === 'file' && !available && <button className="text-button" disabled={busy} onClick={() => onRelink(item)}>{t('重新指向', 'Relink')}</button>}
          {item.kind === 'web' && onDeleteWeb && <button className="icon-button" disabled={busy} title={t('删除网页','Delete link')} aria-label={t('删除网页 ','Delete link ')+item.name} onClick={()=>onDeleteWeb(item)}><Trash2 size={15}/></button>}
          {item.kind === 'web' && <button className="icon-button" disabled={busy} title={t('编辑网页', 'Edit link')} aria-label={t('编辑网页 ', 'Edit link ') + name} onClick={() => onEdit(item)}><MoreHorizontal size={18} /></button>}
        </div>
      </div>;
    })}
    {virtual&&<div aria-hidden="true" style={{height:(items.length-last)*rowHeight}}/>}
    </div>
  </div>;
}

export function EmptyState({ t, loading, view, mode, kind, hasLocations, disabled, onLocation, onAdd, onWeb }: {
  t: Translator; loading: boolean; view: string; mode: string; kind: string; hasLocations: boolean; disabled: boolean;
  onLocation: (suggestion?:SuggestedLocation) => void; onAdd: () => void; onWeb: () => void;
}) {
  const inWorkspace = view === 'workspace';
  const title = loading ? t('正在读取资料', 'Loading resources') : view==='search-empty'?t('输入名称或路径','Search by name or path'):view==='search'?t('没有匹配的资料','No matching resources'):inWorkspace&&kind!=='all'?t('此分类下暂无资料','No resources in this category'):inWorkspace ? t('这个工作区还没有资料', 'No resources here yet') : mode === 'favorites' ? t('还没有收藏的资料', 'No favorites yet') : mode === 'recent' ? t('还没有打开记录', 'No opened resources yet') : hasLocations ? t('这里暂时没有资料', 'No resources yet') : t('文件原地不动，多一个更容易找的入口', 'Your files stay in place. Finding them gets easier.');
  const hint = loading ? t('正在读取已授权的资料。','Reading authorized resources.') : view==='search-empty'?t('输入关键词后查找已授权资料。','Enter a query to find authorized resources.'):view==='search'?t('换个关键词，或调整搜索范围和资料类型。','Try another keyword, scope or resource type.'):inWorkspace&&kind==='web'?t('添加与这件工作有关的网页。','Save a link related to this project.'):inWorkspace ? t('从已授权位置查找文件，或添加与这件工作有关的网页。', 'Find files in managed locations, or save a link for this project.') : mode === 'favorites' ? t('点击资料旁的星标，把常用资料留在这里。', 'Star a resource to keep it close at hand.') : mode === 'recent' ? t('通过 WorkTwill 打开的资料会显示在这里。', 'Resources opened through WorkTwill appear here.') : hasLocations ? t('扫描期间资料会逐步出现，也可以添加其他管理位置。', 'Resources appear as scanning progresses. You can also add another location.') : t('选择一个现有文件夹，即可查找资料，无需先整理或移动文件。', 'Choose an existing folder to find resources. No organizing or moving required.');
  return <div role={loading?'status':undefined} aria-busy={loading} className={`empty-state ${!hasLocations && !inWorkspace ? 'first-use' : ''}`}>
    <div className="empty-symbol">{loading ? <LoaderCircle className="spin" size={28} /> : <FolderOpen size={30} />}</div><h3>{title}</h3><p>{hint}</p>
    {!loading && !view.startsWith('search')&&<div className="actions">{inWorkspace ? <>{kind!=='web'&&<button className="button primary" disabled={disabled} onClick={onAdd}><Plus size={16} />{t('添加文件', 'Add files')}</button>}{kind !== 'file' && <button className="button" disabled={disabled} onClick={onWeb}><Link2 size={16} />{t('添加网页', 'Add link')}</button>}</> : !hasLocations || (mode !== 'favorites' && mode !== 'recent') ? <button className="button primary" disabled={disabled} onClick={()=>onLocation()}><Plus size={16} />{t('添加管理位置', 'Add location')}</button> : null}</div>}
    {!loading && !hasLocations && !inWorkspace && !view.startsWith('search') && <LocationSuggestions t={t} disabled={disabled} onSelect={onLocation}/>}
    {!loading && !hasLocations && !inWorkspace && <div className="onboarding-notes"><span>{t('仅扫描所选位置', 'Only selected locations')}</span><span>{t('支持文件与网页', 'Files and links together')}</span><span>{t('本地存储，无需登录', 'Local, no account needed')}</span></div>}
  </div>;
}
