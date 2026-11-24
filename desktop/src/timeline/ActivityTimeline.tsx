import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { ArrowUpRight, CalendarDays, ChevronLeft, ChevronRight, Clock3, File, FilePlus2, FileText, Folder, FolderOpen, Globe, LoaderCircle, MapPin, RefreshCw, SlidersHorizontal } from 'lucide-react';
import { api, errorMessage, native } from '../api';
import type { CalendarPoint, Resource, TimelineCursor, TimelineEntry, TimelinePage, TimelineRequest, Workspace } from '../contracts';
import './timeline.css';

type Lang = 'zh' | 'en';
type Filter = 'all' | 'modified' | 'discovered' | 'opened' | 'system';
type Basis = 'file_modified' | 'observed';
type Props = {
  lang: Lang;
  // 父级 UI 失效令牌，只触发重读；不是数据库 revision。
  revision: number;
  refreshRevision?: number;
  onOpen: (resource: Resource) => void;
  onReveal: (resource: Resource) => void;
  onWorkspace: (id: string) => void;
  workspaces: Workspace[];
  workspacesReady?: boolean;
};
type Loaded = { key: string; page: TimelinePage };

function dayKey(date: Date): string {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`;
}
function localDay(key: string): Date {
  const [year, month, day] = key.split('-').map(Number);
  return new Date(year, month - 1, day);
}
function isValidTime(time: number): boolean {
  return Number.isFinite(time) && time > 0 && time <= 8_640_000_000_000_000;
}
function sameSnapshot(a: TimelinePage, b: TimelinePage): boolean {
  return a.session === b.session && a.permission_version === b.permission_version && a.revision === b.revision;
}
function calendarCounts(points: CalendarPoint[]): Map<string, number> {
  const counts = new Map<string, number>();
  for (const point of points) {
    const key = dayKey(new Date(point.at));
    counts.set(key, (counts.get(key) ?? 0) + point.count);
  }
  return counts;
}
function resourceStyle(resource: Resource): string {
  if (resource.kind === 'web') return 'web';
  const extension = resource.name.split('.').pop()?.toLowerCase();
  if (extension && ['doc', 'docx', 'wps', 'rtf', 'odt'].includes(extension)) return 'document';
  if (extension && ['xls', 'xlsx', 'et', 'csv', 'ods'].includes(extension)) return 'sheet';
  if (extension === 'pdf') return 'pdf';
  if (extension && ['ppt', 'pptx', 'dps', 'odp'].includes(extension)) return 'slides';
  return 'file';
}
function ResourceIcon({ resource }: { resource: Resource }) {
  const style = resourceStyle(resource);
  const extension = resource.kind === 'web' ? 'URL' : resource.name.split('.').pop()?.toUpperCase();
  const badge = style === 'document' ? 'W' : style === 'sheet' ? 'X' : style === 'pdf' ? 'PDF' : style === 'slides' ? 'P' : null;
  return <span className={`timeline-resource-icon timeline-resource-${style}`} aria-hidden="true">{badge ? <b>{badge}</b> : resource.kind === 'web' ? <Globe size={21} /> : <File size={21} />}{!badge && extension && extension.length <= 5 && resource.kind !== 'web' && <small>{extension}</small>}</span>;
}

export default function ActivityTimeline({ lang, revision, refreshRevision=0, onOpen, onReveal, onWorkspace, workspaces, workspacesReady = true }: Props) {
  const t = useCallback((zh: string, en: string) => lang === 'zh' ? zh : en, [lang]);
  const locale = lang === 'zh' ? 'zh-CN' : 'en-US';
  const [month, setMonth] = useState(() => new Date(new Date().getFullYear(), new Date().getMonth(), 1));
  const [selectedDay, setSelectedDay] = useState<string | null>(null);
  const [filter, setFilter] = useState<Filter>('all');
  const [basis, setBasis] = useState<Basis>('file_modified');
  const [workspaceId, setWorkspaceId] = useState<string | null>(null);
  const [loaded, setLoaded] = useState<Loaded | null>(null);
  const [loading, setLoading] = useState(native);
  const [error, setError] = useState<{cause:unknown}|null>(null);
  const [retry, setRetry] = useState(0);
  const sequence = useRef(0);
  const inFlight = useRef(false);
  const loadedRef = useRef<Loaded | null>(null);
  const today = dayKey(new Date());
  const monthFrom = month.getTime();
  const monthTo = new Date(month.getFullYear(), month.getMonth() + 1, 1).getTime();
  const request = useMemo<TimelineRequest>(() => {
    const date = selectedDay ? localDay(selectedDay) : null;
    const monthDate = new Date(monthFrom);
    const dayCount = new Date(monthDate.getFullYear(), monthDate.getMonth() + 1, 0).getDate();
    return {
      from: date ? date.getTime() : monthFrom,
      // 用本地下一日的午夜形成半开区间，夏令时切换日不固定加 24 小时。
      to: date ? new Date(date.getFullYear(), date.getMonth(), date.getDate() + 1).getTime() : monthTo,
      month_from: monthFrom, month_to: monthTo,
      day_boundaries: Array.from({ length: dayCount + 1 }, (_, index) => new Date(monthDate.getFullYear(), monthDate.getMonth(), index + 1).getTime()),
      filter, basis, workspace_id: workspaceId, offset: 0,
    };
  }, [selectedDay, monthFrom, monthTo, filter, basis, workspaceId]);
  // 查询/强失效令牌决定可见性；普通刷新与手动重试不撤下已确认的活动。
  const key = JSON.stringify([request, revision]);
  const current = loaded?.key === key ? loaded.page : null;

  const load = useCallback(async (append: boolean) => {
    if (!native || (append && inFlight.current)) return;
    const id = ++sequence.current;
    const previous = loadedRef.current?.key === key ? loadedRef.current.page : null;
    inFlight.current = true;
    setLoading(true); setError(null);
    if (!append&&!previous) { setLoaded(null); loadedRef.current = null; }
    const readSnapshot = async (offset: number, cursor?: TimelineCursor): Promise<TimelinePage> => {
      for (let attempt = 0; attempt < 2; attempt++) {
        const metadata = await api.overview();
        if (id !== sequence.current) throw new Error('stale_timeline_request');
        const result = await api.timeline({ ...request, offset, cursor });
        if (id !== sequence.current) throw new Error('stale_timeline_request');
        if (result.session === metadata.session && result.permission_version === metadata.permission_version && result.revision >= metadata.revision) return result;
      }
      throw {message:'资料状态已更新，请刷新后重试',message_en:'Resources changed. Refresh and try again.'};
    };
    try {
      let result: TimelinePage;
      try {
        if (append && previous) {
          const cursor = previous.next_cursor;
          if (!cursor) throw {message:'时间线分页信息无效，请刷新后重试',message_en:'Timeline pagination is unavailable. Refresh and try again.'};
          result = await readSnapshot(cursor.next_offset, cursor);
        } else {
          result = await readSnapshot(0);
          // 普通刷新保留已加载范围，各页必须来自同一版本。
          const target=previous?.events.length??0;
          while(result.has_more&&result.next_cursor&&result.events.length<target){
            const next=await readSnapshot(result.next_cursor.next_offset,result.next_cursor);
            if(!sameSnapshot(result,next))throw {message:'资料状态已更新，请刷新后重试',message_en:'Resources changed. Refresh and try again.'};
            result={...next,events:[...result.events,...next.events]};
          }
        }
      } catch (cause) {
        if (append && typeof cause === 'object' && cause !== null && 'code' in cause && cause.code === 'stale_cursor') {
          // 后端拒绝旧位置时只重读一次首页，不拼接两个读取快照。
          result = await readSnapshot(0);
          append = false;
        } else throw cause;
      }
      // 新事件或授权变化会改变 offset；不同快照必须从第一页重读，不能混合拼接。
      if (append && previous && !sameSnapshot(previous, result)) {
        result = await readSnapshot(0);
        append = false;
      }
      if (id !== sequence.current) return;
      const page = append && previous ? { ...result, events: [...previous.events, ...result.events] } : result;
      const value = { key, page };
      loadedRef.current = value; setLoaded(value);
    } catch (cause) {
      if (id === sequence.current) { setLoaded(null); loadedRef.current = null; setError({cause}); }
    } finally {
      if (id === sequence.current) { inFlight.current = false; setLoading(false); }
    }
  }, [key, request, retry, refreshRevision]);

  useEffect(() => {
    if (!native) { setLoading(false); return; }
    void load(false);
    return () => { sequence.current++; inFlight.current = false; };
  }, [load]);
  useEffect(() => {
    // 重读期间的空列表不代表删除；新快照确认不存在后才扩大到全部工作区。
    if (workspacesReady && workspaceId && !workspaces.some(item => item.id === workspaceId)) setWorkspaceId(null);
  }, [workspaceId, workspaces, workspacesReady]);

  const counts = useMemo(() => calendarCounts(current?.calendar ?? []), [current?.calendar]);
  const days = useMemo(() => {
    const start = (month.getDay() + 6) % 7;
    const count = new Date(month.getFullYear(), month.getMonth() + 1, 0).getDate();
    return Array.from({ length: Math.ceil((start + count) / 7) * 7 }, (_, index) => index >= start && index < start + count ? new Date(month.getFullYear(), month.getMonth(), index - start + 1) : null);
  }, [month]);
  const groups = useMemo(() => {
    const result = new Map<string, TimelineEntry[]>();
    for (const entry of current?.events ?? []) {
      const group = dayKey(new Date(entry.effective_at));
      const entries = result.get(group) ?? []; entries.push(entry); result.set(group, entries);
    }
    return Array.from(result.entries());
  }, [current?.events]);
  const recentResources = useMemo(() => {
    const result: Resource[] = []; const seen = new Set<string>();
    for (const { resource } of current?.events ?? []) {
      if (resource && !seen.has(resource.id)) { result.push(resource); seen.add(resource.id); }
      if (result.length === 5) break;
    }
    return result;
  }, [current?.events]);
  const monthCount = Array.from(counts.values()).reduce((sum, count) => sum + count, 0);
  const dateFormat = (date: Date, options: Intl.DateTimeFormatOptions) => date.toLocaleDateString(locale, options);
  const fullDate = (time: number) => new Date(time).toLocaleString(locale, { year: 'numeric', month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' });
  const dayLabel = (key: string) => {
    const date = localDay(key);
    const yesterday = new Date(); yesterday.setDate(yesterday.getDate() - 1);
    const relative = key === today ? t('今天', 'Today') : key === dayKey(yesterday) ? t('昨天', 'Yesterday') : '';
    return `${relative ? relative + ' · ' : ''}${dateFormat(date, { month: 'long', day: 'numeric' })}`;
  };
  const eventLabel = (entry: TimelineEntry) => {
    const labels: Record<string, string> = {
      file_modified: t('文件修改', 'File modified'), file_discovered: t('首次发现', 'First discovered'),
      opened: t('应用内打开', 'Opened in WorkTwill'), linked: t('加入工作区', 'Added to workspace'), unlinked: t('移除关联', 'Reference removed'),
      transferred: t('转移引用', 'Reference transferred'), workspace_created: t('创建工作区', 'Workspace created'), workspace_deleted: t('删除工作区', 'Workspace deleted'),
      workspace_renamed: t('工作区改名', 'Workspace renamed'), web_saved: t('保存网页', 'Link saved'), relinked: t('重新指向', 'Reference repaired'),
    };
    return labels[entry.activity.kind] ?? t('应用操作', 'App activity');
  };
  const moveMonth = (direction: number) => { setMonth(previous => new Date(previous.getFullYear(), previous.getMonth() + direction, 1)); setSelectedDay(null); };
  const goToday = () => { const date = new Date(); setMonth(new Date(date.getFullYear(), date.getMonth(), 1)); setSelectedDay(dayKey(date)); };
  const filterLabels: Array<[Filter, string]> = [['all', t('全部', 'All')], ['modified', t('文件修改', 'Modified')], ['discovered', t('首次发现', 'Discovered')], ['opened', t('应用内打开', 'Opened')], ['system', t('应用操作', 'App actions')]];
  const statusLabel = (resource: Resource) => resource.status === 'available' ? '' : resource.status === 'identity_unconfirmed' ? t('身份待确认', 'Identity unconfirmed') : t('原件暂不可用', 'Original unavailable');

  return <section className="timeline-view" aria-label={t('文件活动时间线', 'File activity timeline')}>
    <aside className="timeline-calendar-panel">
      <div className="timeline-month-navigation"><button type="button" aria-label={t('上个月', 'Previous month')} onClick={() => moveMonth(-1)}><ChevronLeft size={18} /></button><h2>{dateFormat(month, { month: 'long', year: 'numeric' })}</h2><button type="button" aria-label={t('下个月', 'Next month')} onClick={() => moveMonth(1)}><ChevronRight size={18} /></button></div>
      <div className="timeline-calendar-top"><button type="button" className={!selectedDay ? 'is-active' : ''} onClick={() => setSelectedDay(null)}>{t('整月', 'Whole month')}</button><button type="button" onClick={goToday}>{t('今天', 'Today')}</button></div>
      <div className="timeline-calendar-weekdays" aria-hidden="true">{(lang === 'zh' ? ['一', '二', '三', '四', '五', '六', '日'] : ['M', 'T', 'W', 'T', 'F', 'S', 'S']).map((label, index) => <span key={index}>{label}</span>)}</div>
      <div className="timeline-calendar-grid" role="group" aria-label={t('选择日期', 'Choose date')}>
        {days.map((date, index) => {
          if (!date) return <span key={`empty-${index}`} />;
          const key = dayKey(date); const count = counts.get(key) ?? 0;
          return <button type="button" key={key} aria-pressed={selectedDay === key} aria-current={key === today ? 'date' : undefined} aria-label={`${dateFormat(date, { year: 'numeric', month: 'long', day: 'numeric' })}${count ? t(`，${count} 条记录`, `, ${count} activities`) : ''}`} title={count ? t(`${count} 条记录`, `${count} activities`) : undefined} className={`${selectedDay === key ? 'is-selected' : ''} ${key === today ? 'is-today' : ''}`} onClick={() => setSelectedDay(old => old === key ? null : key)}><span>{date.getDate()}</span>{count > 0 && <i aria-hidden="true" />}</button>;
        })}
      </div>
      <p className="timeline-calendar-note">{current ? t(`本月 ${monthCount} 条记录`, `${monthCount} activities this month`) : error !== null ? t('活动日期暂不可用', 'Activity dates unavailable') : loading ? t('正在读取活动日期…', 'Loading activity dates…') : t('尚未读取活动日期', 'Activity dates have not been loaded')}</p>
      <div className="timeline-sidebar-section"><h3><SlidersHorizontal size={15} />{t('活动筛选', 'Activity filters')}</h3><div className="timeline-filter-list" role="group" aria-label={t('活动类型', 'Activity type')}>{filterLabels.map(([value, label]) => <button type="button" key={value} aria-pressed={filter === value} className={filter === value ? 'is-active' : ''} onClick={() => setFilter(value)}>{label}</button>)}</div></div>
      <div className="timeline-sidebar-section"><label className="timeline-field-label" htmlFor="timeline-workspace-filter">{t('工作区', 'Workspace')}</label><select id="timeline-workspace-filter" value={workspaceId ?? ''} onChange={event => setWorkspaceId(event.target.value || null)}><option value="">{t('全部工作区', 'All workspaces')}</option>{workspaces.map(workspace => <option key={workspace.id} value={workspace.id}>{workspace.name}</option>)}</select></div>
      <div className="timeline-sidebar-section"><label className="timeline-field-label" htmlFor="timeline-time-basis">{t('时间维度', 'Time basis')}</label><select id="timeline-time-basis" value={basis} onChange={event => setBasis(event.target.value as Basis)}><option value="file_modified">{t('文件修改时间', 'File modification time')}</option><option value="observed">{t('本应用发现时间', 'Observed in WorkTwill')}</option></select><p className="timeline-scope-note">{t('首次发现按收纳日期显示。工作区筛选按当前关联。', 'Discoveries use their collection date. Workspace filters use current references.')}</p></div>
    </aside>

    <div className="timeline-center">
      <header className="timeline-heading"><div><p className="timeline-eyebrow">{t('你的工作足迹', 'YOUR WORK, OVER TIME')}</p><h2>{t('活动时间线', 'Activity timeline')}</h2><p>{basis === 'file_modified' ? t('按文件最后修改时间回顾，发现与打开按应用记录时间。', 'Review file modification times; discoveries and opens use app record time.') : t('按本应用发现与操作时间，找回当时的工作资料。', 'Find your work by when WorkTwill observed or opened it.')}</p></div><button type="button" className="timeline-refresh" disabled={loading || !native} aria-label={t('刷新时间线', 'Refresh timeline')} onClick={() => setRetry(value => value + 1)}><RefreshCw size={16} className={loading ? 'spin' : ''} /></button></header>
      <div className="timeline-current-range"><CalendarDays size={15} /><span>{selectedDay ? dateFormat(localDay(selectedDay), { year: 'numeric', month: 'long', day: 'numeric', weekday: 'short' }) : dateFormat(month, { year: 'numeric', month: 'long' })}</span>{workspaceId && <span className="timeline-range-chip">{workspaces.find(item => item.id === workspaceId)?.name}</span>}</div>
      {!native ? <div className="timeline-empty"><CalendarDays size={32} /><h3>{t('在桌面应用查看真实活动', 'View real activity in the desktop app')}</h3><p>{t('浏览器预览不读取本地文件，也不生成演示历史。', 'The browser preview does not read local files or generate demo history.')}</p></div> : error !== null ? <div className="timeline-error" role="alert"><p>{errorMessage(error.cause,lang)}</p><button type="button" onClick={() => setRetry(value => value + 1)}>{t('重新加载', 'Try again')}</button></div> : loading && !current ? <div className="timeline-empty" role="status"><LoaderCircle size={28} className="spin" /><h3>{t('正在读取活动', 'Loading activity')}</h3><p>{t('正在核对当前授权范围。', 'Checking your current authorized scope.')}</p></div> : !current?.events.length ? <div className="timeline-empty"><Clock3 size={32} /><h3>{selectedDay ? t('这一天暂无活动记录', 'No activity for this day') : t('这个月暂无活动记录', 'No activity for this month')}</h3><p>{t('仅展示已授权资料的可靠记录；可以更换日期或筛选条件。', 'Only reliable records for authorized resources appear here. Try another date or filter.')}</p>{selectedDay && <button type="button" onClick={() => setSelectedDay(null)}>{t('查看整月', 'View whole month')}</button>}</div> : <div className="timeline-days">
        {groups.map(([key, entries]) => <section className="timeline-day" key={key} aria-label={dayLabel(key)}><h3 className="timeline-day-label"><CalendarDays size={17} />{dayLabel(key)}</h3><div className="timeline-day-events">{entries.map(entry => {
          const resource = entry.resource; const isFileEvent = entry.activity.source === 'filesystem';
          // 历史标题使用观察时名称；打开和定位仍指向当前资源，不表示旧物理版本。
          const title = entry.activity.name || resource?.name || '';
          const related = resource ? workspaces.filter(workspace => resource.workspace_ids.includes(workspace.id)) : [];
          const available = resource?.status === 'available';
          return <article className={`timeline-entry ${entry.activity.kind === 'file_discovered' ? 'timeline-entry-discovered' : ''}`} key={entry.activity.id}>
            <time className="timeline-entry-time" dateTime={new Date(entry.effective_at).toISOString()}>{new Date(entry.effective_at).toLocaleTimeString(locale, { hour: '2-digit', minute: '2-digit' })}</time><div className="timeline-event-card">
              {resource ? <ResourceIcon resource={resource} /> : <span className="timeline-resource-icon timeline-resource-app"><Folder size={21} /></span>}
              <div className="timeline-event-copy"><div className="timeline-event-title"><span className={`timeline-event-kind timeline-kind-${entry.activity.kind}`}>{eventLabel(entry)}</span><strong title={title}>{title}</strong></div>
                <div className="timeline-event-meta"><span>{isFileEvent ? t('来自文件系统', 'From file system') : t('来自本应用', 'From WorkTwill')}</span>{related.slice(0, 2).map(workspace => <button type="button" key={workspace.id} onClick={() => onWorkspace(workspace.id)}>{workspace.name}</button>)}{related.length > 2 && <span>+{related.length - 2}</span>}{resource && !available && <span className="timeline-unavailable">{statusLabel(resource)}</span>}</div>
                {isFileEvent && <p className="timeline-observation-note">{entry.activity.kind === 'file_modified' && isValidTime(entry.activity.file_modified_at ?? 0) ? <><span>{t('文件最后修改：', 'File modified: ')}{fullDate(entry.activity.file_modified_at!)}</span><span>{t('本应用发现：', 'Observed: ')}{fullDate(entry.activity.at)}</span></> : entry.activity.kind === 'file_discovered' ? <><span>{t('首次发现，不代表文件创建时间', 'First discovered, not the file creation time')}</span>{isValidTime(entry.activity.file_modified_at ?? 0) && <span>{t('文件最后修改：', 'File modified: ')}{fullDate(entry.activity.file_modified_at!)}</span>}</> : <span>{t('本应用发现：', 'Observed: ')}{fullDate(entry.activity.at)}</span>}</p>}
              </div>
              {resource && <div className="timeline-event-actions"><button type="button" disabled={!available} onClick={() => onOpen(resource)} aria-label={t('打开 ', 'Open ') + resource.name}><ArrowUpRight size={15} /><span>{resource.kind === 'web' ? t('打开网页', 'Open link') : t('打开文件', 'Open file')}</span></button>{resource.kind === 'file' && <button type="button" disabled={!available} onClick={() => onReveal(resource)} aria-label={t('定位 ', 'Reveal ') + resource.name}><MapPin size={15} /><span>{t('定位', 'Reveal')}</span></button>}</div>}
            </div>
          </article>;
        })}</div></section>)}
        {current.has_more && <button type="button" className="timeline-load-more" disabled={loading} onClick={() => void load(true)}>{loading ? <LoaderCircle size={16} className="spin" /> : <ChevronRight size={16} />}{loading ? t('正在读取…', 'Loading…') : t('加载更多活动', 'Load more activity')}</button>}
      </div>}
      <p className="timeline-evidence-note">{t('应用关闭期间只保留再次扫描确认的最新变化，不还原逐次编辑历史。', 'Changes while the app is closed are confirmed at the next scan. Individual edits cannot be reconstructed.')}</p>
    </div>

    <aside className="timeline-quick-panel"><h2>{t('快速返回', 'Quick return')}</h2><div className="timeline-quick-section"><h3><FolderOpen size={16} />{t('我的工作区', 'My workspaces')}</h3>{workspaces.length ? workspaces.slice(0, 6).map((workspace, index) => <button type="button" className="timeline-quick-workspace" data-workspace-id={workspace.id} key={workspace.id} onClick={() => onWorkspace(workspace.id)}><span className={`timeline-folder-tone timeline-folder-${index % 4}`}><Folder size={20} fill="currentColor" strokeWidth={1.4} /></span><span>{workspace.name}</span><ChevronRight size={15} /></button>) : <p className="timeline-quick-empty">{t('创建工作区后，可从这里继续。', 'Your workspaces will appear here.')}</p>}</div><div className="timeline-quick-section"><h3><FileText size={16} />{t('最近活动资料', 'Recent activity resources')}</h3>{recentResources.length ? recentResources.map(resource => <button type="button" className="timeline-quick-resource" key={resource.id} disabled={resource.status !== 'available'} onClick={() => onOpen(resource)}><ResourceIcon resource={resource} /><span title={resource.name}>{resource.name}</span><ChevronRight size={15} /></button>) : <p className="timeline-quick-empty">{error !== null ? t('活动资料暂不可用。', 'Activity resources unavailable.') : !current ? t('尚未读取活动资料。', 'Activity resources have not been loaded.') : t('当前时间范围内暂无资料。', 'No resources in this date range.')}</p>}</div><p className="timeline-quick-footnote">{t('只显示当前已授权的资料', 'Only currently authorized resources')}</p></aside>
  </section>;
}
