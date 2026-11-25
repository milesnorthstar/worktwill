import { act, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Overview, Resource, TimelineCursor, TimelineEntry, TimelinePage, TimelineRequest } from '../contracts';

const mocks = vi.hoisted(() => ({ timeline: vi.fn(), overview: vi.fn() }));
vi.mock('../api', async importOriginal => ({ ...(await importOriginal<typeof import('../api')>()), native: true, api: mocks }));
import ActivityTimeline from './ActivityTimeline';

const metadata: Overview = { session: 's', permission_version: 1, revision: 1, sqlite_version: 'test', roots: [], file_grants: [], workspaces: [], activities: [] };
const file: Resource = { id: 'file1', name: '合同_验收.docx', kind: 'file', location: '/fixture/合同_验收.docx', status: 'available', modified_at: 1, size: 1, favorite: false, workspace_ids: ['w'] };
const props = { lang: 'zh' as const, revision: 1, onOpen: vi.fn(), onReveal: vi.fn(), onWorkspace: vi.fn(), workspaces: [{ id: 'w', name: '合同资料', count: 1 }] };
const now = new Date();
const at = new Date(now.getFullYear(), now.getMonth(), 3, 14, 25).getTime();
function entry(id: string, extra: Partial<TimelineEntry> = {}): TimelineEntry {
  return { activity: { id, kind: 'file_modified', name: extra.resource?.name ?? file.name, resource_id: file.id, at: at + 3_600_000, source: 'filesystem', file_modified_at: at }, resource: file, effective_at: at, ...extra };
}
function page(events: TimelineEntry[] = [], extra: Partial<TimelinePage> = {}): TimelinePage {
  return { session: 's', permission_version: 1, revision: 1, events, calendar: [], has_more: false, ...extra };
}
function cursorFor(request: TimelineRequest, last: TimelineEntry): TimelineCursor {
  return {version:1,session:'s',permission_version:1,revision:1,next_offset:100,before:{at:last.effective_at,id:last.activity.id},criteria:{from:request.from,to:request.to,month_from:request.month_from,month_to:request.month_to,day_boundaries:request.day_boundaries,filter:request.filter,basis:request.basis,workspace_id:request.workspace_id}};
}
beforeEach(() => {
  vi.clearAllMocks();
  mocks.timeline.mockReset(); mocks.overview.mockReset();
  mocks.overview.mockResolvedValue(metadata); mocks.timeline.mockResolvedValue(page());
});

afterEach(() => vi.useRealTimers());

describe('文件时间线日期与来源', () => {
  it('夏令时切换日与跨年月份均按本地日历构造边界', async () => {
    vi.useFakeTimers({ toFake: ['Date'] });
    vi.setSystemTime(new Date(2026, 2, 12, 12));
    const user = userEvent.setup(); render(<ActivityTimeline {...props} />);
    await screen.findByText('这个月暂无活动记录');
    const day = new Date(2026, 2, 8);
    await user.click(screen.getByRole('button', { name: day.toLocaleDateString('zh-CN', { year: 'numeric', month: 'long', day: 'numeric' }) }));
    await waitFor(() => expect(mocks.timeline).toHaveBeenCalledTimes(2));
    const request = mocks.timeline.mock.calls[1][0] as TimelineRequest;
    expect(request.day_boundaries).toHaveLength(32);
    expect(request.day_boundaries[7]).toBe(new Date(2026, 2, 8).getTime());
    expect(request.day_boundaries[8]).toBe(new Date(2026, 2, 9).getTime());
    expect(request.day_boundaries.at(-1)).toBe(new Date(2026, 3, 1).getTime());
    if (Intl.DateTimeFormat().resolvedOptions().timeZone === 'America/New_York') {
      expect(request.day_boundaries[8] - request.day_boundaries[7]).toBe(23 * 3_600_000);
    }
    expect(request.from).toBe(new Date(2026, 2, 8).getTime());
    expect(request.to).toBe(new Date(2026, 2, 9).getTime());
    expect(request.to! - request.from!).toBe(new Date(2026, 2, 9).getTime() - new Date(2026, 2, 8).getTime());
    for (let index = 0; index < 3; index++) await user.click(screen.getByRole('button', { name: '上个月' }));
    await waitFor(() => expect(mocks.timeline.mock.calls.at(-1)?.[0]).toMatchObject({ month_from: new Date(2025, 11, 1).getTime(), month_to: new Date(2026, 0, 1).getTime() }));
  });
  it('默认按文件修改时间请求本地整月，选日使用本地下一日的半开边界', async () => {
    mocks.timeline.mockResolvedValue(page([entry('a')], { calendar: [{ at, count: 125 }] }));
    const user = userEvent.setup(); render(<ActivityTimeline {...props} />);
    await screen.findByRole('article');
    const initial = mocks.timeline.mock.calls[0][0] as TimelineRequest;
    expect(initial.basis).toBe('file_modified');
    expect(initial.from).toBe(new Date(now.getFullYear(), now.getMonth(), 1).getTime());
    expect(initial.to).toBe(new Date(now.getFullYear(), now.getMonth() + 1, 1).getTime());
    expect(initial.day_boundaries[0]).toBe(initial.month_from);
    expect(initial.day_boundaries.at(-1)).toBe(initial.month_to);
    expect(initial.day_boundaries.length).toBeLessThanOrEqual(32);
    expect(screen.getByText('本月 125 条记录')).toBeVisible();
    const date = new Date(at);
    const label = date.toLocaleDateString('zh-CN', { year: 'numeric', month: 'long', day: 'numeric' }) + '，125 条记录';
    await user.click(screen.getByRole('button', { name: label }));
    await waitFor(() => expect(mocks.timeline).toHaveBeenCalledTimes(2));
    const selected = mocks.timeline.mock.calls[1][0] as TimelineRequest;
    expect(selected.from).toBe(new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime());
    expect(selected.to).toBe(new Date(date.getFullYear(), date.getMonth(), date.getDate() + 1).getTime());
    expect(selected.day_boundaries).toEqual(initial.day_boundaries);
    expect(screen.getByRole('button', { name: label })).toHaveAttribute('aria-pressed', 'true');
  });

  it('秋季夏令时和闰年二月的日历边界保持完整且严格递增', async () => {
    vi.useFakeTimers({ toFake: ['Date'] });
    vi.setSystemTime(new Date(2024, 10, 4, 12));
    const { unmount } = render(<ActivityTimeline {...props} />);
    await screen.findByText('这个月暂无活动记录');
    const november = mocks.timeline.mock.calls[0][0] as TimelineRequest;
    expect(november.day_boundaries).toHaveLength(31);
    expect(november.day_boundaries[2]).toBe(new Date(2024, 10, 3).getTime());
    expect(november.day_boundaries[3]).toBe(new Date(2024, 10, 4).getTime());
    if (Intl.DateTimeFormat().resolvedOptions().timeZone === 'America/New_York') {
      expect(november.day_boundaries[3] - november.day_boundaries[2]).toBe(25 * 3_600_000);
    }
    unmount(); mocks.timeline.mockClear();
    vi.setSystemTime(new Date(2024, 1, 15, 12));
    render(<ActivityTimeline {...props} />);
    await screen.findByText('这个月暂无活动记录');
    const february = mocks.timeline.mock.calls[0][0] as TimelineRequest;
    expect(february.day_boundaries).toHaveLength(30);
    expect(february.day_boundaries.at(-1)).toBe(new Date(2024, 2, 1).getTime());
    expect(february.day_boundaries.every((time, index, values) => index === 0 || time > values[index - 1])).toBe(true);
  });

  it('卡片区分文件修改与观察时间，打开/定位/工作区动作由上层回调处理', async () => {
    mocks.timeline.mockResolvedValue(page([entry('a')]));
    const user = userEvent.setup(); render(<ActivityTimeline {...props} />);
    const article = await screen.findByRole('article');
    expect(within(article).getByText('来自文件系统')).toBeVisible();
    expect(within(article).getByText(/文件最后修改：/)).toBeVisible();
    expect(within(article).getByText(/本应用发现：/)).toBeVisible();
    await user.click(within(article).getByRole('button', { name: '打开 ' + file.name }));
    await user.click(within(article).getByRole('button', { name: '定位 ' + file.name }));
    await user.click(within(article).getByRole('button', { name: '合同资料' }));
    expect(props.onOpen).toHaveBeenCalledExactlyOnceWith(file);
    expect(props.onReveal).toHaveBeenCalledExactlyOnceWith(file);
    expect(props.onWorkspace).toHaveBeenCalledExactlyOnceWith('w');
  });

  it('首次发现明确不代表创建时间；不可用原件没有可执行打开入口', async () => {
    const discovered = entry('new', { activity: { ...entry('new').activity, kind: 'file_discovered' }, resource: { ...file, status: 'missing' } });
    mocks.timeline.mockResolvedValue(page([discovered])); render(<ActivityTimeline {...props} />);
    const article = await screen.findByRole('article');
    expect(within(article).getByText('首次发现，不代表文件创建时间')).toBeVisible();
    expect(within(article).getByRole('button', { name: '打开 ' + file.name })).toBeDisabled();
    expect(within(article).getByRole('button', { name: '定位 ' + file.name })).toBeDisabled();
  });

  it('跨月清空选日，应用操作与工作区筛选明确传给后端', async () => {
    const user = userEvent.setup(); render(<ActivityTimeline {...props} />);
    await screen.findByText('这个月暂无活动记录');
    await user.click(screen.getByRole('button', { name: '今天' }));
    await waitFor(() => expect(mocks.timeline).toHaveBeenCalledTimes(2));
    await user.click(screen.getByRole('button', { name: '上个月' }));
    await waitFor(() => expect(mocks.timeline).toHaveBeenCalledTimes(3));
    const prior = mocks.timeline.mock.calls[2][0] as TimelineRequest;
    expect(prior.from).toBe(new Date(now.getFullYear(), now.getMonth() - 1, 1).getTime());
    expect(prior.to).toBe(new Date(now.getFullYear(), now.getMonth(), 1).getTime());
    await user.click(screen.getByRole('button', { name: '应用操作' }));
    await user.click(screen.getByRole('combobox', { name: '工作区' }));
    await user.click(screen.getByRole('option', { name: '合同资料' }));
    await waitFor(() => expect(mocks.timeline.mock.calls.at(-1)?.[0]).toMatchObject({ filter: 'system', workspace_id: 'w', offset: 0 }));
  });
});

describe('时间线版本与错误保护', () => {
  it('工作区列表重读不丢失所选范围，确认删除后才重置', async () => {
    const user = userEvent.setup();
    const { rerender } = render(<ActivityTimeline {...props} />);
    await screen.findByText('这个月暂无活动记录');
    await user.click(screen.getByRole('combobox', { name: '工作区' }));
    await user.click(screen.getByRole('option', { name: '合同资料' }));
    await waitFor(() => expect(mocks.timeline.mock.calls.at(-1)?.[0]).toMatchObject({ workspace_id: 'w' }));
    const callsBeforeReload = mocks.timeline.mock.calls.length;

    rerender(<ActivityTimeline {...props} workspaces={[]} workspacesReady={false} />);
    rerender(<ActivityTimeline {...props} workspacesReady />);
    expect(screen.getByRole('combobox', { name: '工作区' })).toHaveTextContent('合同资料');
    expect(mocks.timeline).toHaveBeenCalledTimes(callsBeforeReload);
    expect(mocks.timeline.mock.calls.at(-1)?.[0]).toMatchObject({ workspace_id: 'w' });

    rerender(<ActivityTimeline {...props} workspaces={[]} workspacesReady />);
    await waitFor(() => expect(mocks.timeline.mock.calls.at(-1)?.[0]).toMatchObject({ workspace_id: null }));
    expect(screen.getByRole('combobox', { name: '工作区' })).toHaveTextContent('全部工作区');
  });

  it('父级失效令牌不是数据库修订，令牌999仍可读取DB revision1', async () => {
    mocks.timeline.mockResolvedValue(page([entry('visible')]));
    const { rerender } = render(<ActivityTimeline {...props} revision={999} />);
    expect(await screen.findByRole('article')).toBeVisible();
    expect(mocks.timeline).toHaveBeenCalledTimes(1);
    rerender(<ActivityTimeline {...props} revision={1000} />);
    await waitFor(() => expect(mocks.timeline).toHaveBeenCalledTimes(2));
    expect(await screen.findByRole('article')).toBeVisible();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });
  it('晚到的旧筛选响应不能覆盖新筛选的事件或月历', async () => {
    const pending: Array<{ request: TimelineRequest; resolve: (value: TimelinePage) => void }> = [];
    mocks.timeline.mockImplementation(request => new Promise(resolve => pending.push({ request, resolve })));
    const user = userEvent.setup(); render(<ActivityTimeline {...props} />);
    await waitFor(() => expect(pending).toHaveLength(1));
    await user.click(screen.getByRole('button', { name: '文件修改' }));
    await waitFor(() => expect(pending).toHaveLength(2));
    await act(async () => pending[1].resolve(page([entry('current', { resource: { ...file, name: '新筛选资料' } })], { calendar: [{ at, count: 3 }] })));
    await screen.findByText('新筛选资料', { selector: 'strong' });
    await act(async () => pending[0].resolve(page([entry('old', { resource: { ...file, name: '过期资料' } })], { calendar: [{ at, count: 99 }] })));
    expect(screen.queryByText('过期资料')).not.toBeInTheDocument();
    expect(screen.getByText('本月 3 条记录')).toBeVisible();
  });

  it('父级失效令牌变更立即清空旧资源，并拒绝旧授权版本返回', async () => {
    mocks.timeline.mockResolvedValueOnce(page([entry('visible')]));
    const { rerender } = render(<ActivityTimeline {...props} />);
    await screen.findByRole('article');
    mocks.overview.mockResolvedValue({ ...metadata, revision: 2, permission_version: 2 });
    mocks.timeline.mockResolvedValue(page([entry('denied', { resource: { ...file, name: '不应泄露' } })], { revision: 2, permission_version: 1 }));
    rerender(<ActivityTimeline {...props} revision={2} />);
    expect(screen.queryByRole('article')).not.toBeInTheDocument();
    await screen.findByRole('alert');
    expect(screen.queryByText('不应泄露')).not.toBeInTheDocument();
    expect(screen.queryByText('本月 1 条记录')).not.toBeInTheDocument();
    expect(mocks.timeline).toHaveBeenCalledTimes(3);
  });

  it('分页响应修订变化时重读第一页，不拼接不同快照', async () => {
    const first = Array.from({ length: 100 }, (_, index) => entry('old-' + index));
    mocks.timeline.mockImplementationOnce((request:TimelineRequest) => Promise.resolve(page(first, { has_more: true, next_cursor: cursorFor(request,first[99]) })))
      .mockResolvedValueOnce(page([entry('appended')], { revision: 2 }))
      .mockResolvedValueOnce(page([entry('fresh', { resource: { ...file, name: '更新后的首页' } })], { revision: 2 }));
    const user = userEvent.setup(); render(<ActivityTimeline {...props} />);
    await user.click(await screen.findByRole('button', { name: '加载更多活动' }));
    await screen.findByText('更新后的首页', { selector: 'strong' });
    expect(mocks.timeline.mock.calls.map(call => call[0].offset)).toEqual([0, 100, 0]);
    expect(screen.getAllByRole('article')).toHaveLength(1);
  });

  it('后续页游标失效只重读一次首页，并原样回传后端位置', async () => {
    const first=Array.from({length:100},(_,index)=>entry('prior-'+index));
    let issued: TimelineCursor | undefined;
    mocks.timeline.mockImplementationOnce((request:TimelineRequest)=>{issued=cursorFor(request,first[99]);return Promise.resolve(page(first,{has_more:true,next_cursor:issued}));})
      .mockRejectedValueOnce({code:'stale_cursor',message:'已过期',message_en:'Expired.'})
      .mockResolvedValueOnce(page([entry('current',{resource:{...file,name:'当前快照'}})],{revision:2}));
    const user=userEvent.setup();render(<ActivityTimeline {...props} />);
    await user.click(await screen.findByRole('button',{name:'加载更多活动'}));
    await screen.findByText('当前快照',{selector:'strong'});
    expect(mocks.timeline.mock.calls.map(call=>call[0].offset)).toEqual([0,100,0]);
    expect(mocks.timeline.mock.calls[1][0].cursor).toBe(issued);
    expect(mocks.timeline.mock.calls[2][0].cursor).toBeUndefined();
    expect(screen.getAllByRole('article')).toHaveLength(1);
  });

  it('旧续页晚到后仍使用新筛选的游标继续加载', async () => {
    const first=Array.from({length:100},(_,index)=>entry('prior-'+index));
    const current=Array.from({length:100},(_,index)=>entry('current-'+index));
    let resolveOld: ((value:TimelinePage)=>void) | undefined;
    let currentCursor: TimelineCursor | undefined;
    mocks.timeline.mockImplementationOnce((q:TimelineRequest)=>Promise.resolve(page(first,{has_more:true,next_cursor:cursorFor(q,first[99])})))
      .mockImplementationOnce(()=>new Promise<TimelinePage>(resolve=>{resolveOld=resolve;}))
      .mockImplementationOnce((q:TimelineRequest)=>{currentCursor=cursorFor(q,current[99]);return Promise.resolve(page(current,{has_more:true,next_cursor:currentCursor}));})
      .mockResolvedValueOnce(page([entry('final',{resource:{...file,name:'新筛选的后续资料'}})]));
    const user=userEvent.setup();render(<ActivityTimeline {...props} />);
    await user.click(await screen.findByRole('button',{name:'加载更多活动'}));
    await waitFor(()=>expect(mocks.timeline).toHaveBeenCalledTimes(2));
    await user.click(screen.getByRole('button',{name:'文件修改'}));
    await waitFor(()=>expect(mocks.timeline).toHaveBeenCalledTimes(3));
    await screen.findByRole('button',{name:'加载更多活动'});
    expect(resolveOld).toBeTypeOf('function');
    await act(async()=>resolveOld!(page([entry('late',{resource:{...file,name:'过期续页'}})],{has_more:true})));
    expect(screen.queryByText('过期续页')).not.toBeInTheDocument();
    await user.click(screen.getByRole('button',{name:'加载更多活动'}));
    await screen.findByText('新筛选的后续资料',{selector:'strong'});
    expect(mocks.timeline.mock.calls[3][0]).toMatchObject({offset:100,filter:'modified'});
    expect(mocks.timeline.mock.calls[3][0].cursor).toBe(currentCursor);
  });

  it('首页游标错误明确报错并且不循环重试', async () => {
    mocks.timeline.mockRejectedValue({code:'stale_cursor',message:'时间线条件已变化',message_en:'Timeline conditions changed.'});
    render(<ActivityTimeline {...props} />);
    expect(await screen.findByRole('alert')).toHaveTextContent('时间线条件已变化');
    expect(mocks.timeline).toHaveBeenCalledTimes(1);
  });

  it('加载失败显示可重试状态，成功后恢复真实空态且英文不泄露内部码', async () => {
    mocks.timeline.mockRejectedValueOnce({message:'磁盘暂不可用',message_en:'The disk is unavailable.'}).mockResolvedValueOnce(page());
    const user = userEvent.setup(); render(<ActivityTimeline {...props} lang="en" />);
    expect(await screen.findByRole('alert')).toHaveTextContent('The disk is unavailable.');
    expect(screen.getByText('Activity dates unavailable')).toBeVisible();
    expect(screen.getByText('Activity resources unavailable.')).toBeVisible();
    expect(screen.queryByText('0 activities this month')).not.toBeInTheDocument();
    expect(screen.queryByText('No resources in this date range.')).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Try again' }));
    expect(await screen.findByText('No activity for this month')).toBeVisible();
    expect(screen.getByText('0 activities this month')).toBeVisible();
    expect(screen.getByText('No resources in this date range.')).toBeVisible();
    expect(screen.getByRole('combobox', { name: 'Time basis' })).toHaveTextContent('File modification time');
    expect(screen.queryByText('file_discovered')).not.toBeInTheDocument();
  });
});


it('时间线失败随语言更新，保持日期筛选且仅明确重试才重读',async()=>{
  mocks.timeline.mockResolvedValueOnce(page()).mockRejectedValueOnce({message:'磁盘暂不可用',message_en:'The disk is unavailable.'}).mockResolvedValueOnce(page());
  const user=userEvent.setup();const {rerender}=render(<ActivityTimeline {...props}/>);
  await screen.findByText('这个月暂无活动记录');
  const date=new Date(now.getFullYear(),now.getMonth(),3);
  await user.click(screen.getByRole('button',{name:date.toLocaleDateString('zh-CN',{year:'numeric',month:'long',day:'numeric'})}));
  expect(await screen.findByRole('alert')).toHaveTextContent('磁盘暂不可用');
  const request=mocks.timeline.mock.calls[1][0];
  rerender(<ActivityTimeline {...props} lang="en"/>);
  expect(screen.getByRole('alert')).toHaveTextContent('The disk is unavailable.');
  expect(mocks.timeline).toHaveBeenCalledTimes(2);
  await user.click(screen.getByRole('button',{name:'Try again'}));
  await screen.findByText('No activity for this day');
  expect(mocks.timeline).toHaveBeenCalledTimes(3);
  expect(mocks.timeline.mock.calls[2][0]).toEqual(request);
});

it('历史标题保留观察时旧名，中英文打开定位及快速返回始终使用当前资源',async()=>{
  const current={...file,name:'合同_定稿.docx',location:'/fixture/current/合同_定稿.docx'};
  const historical=entry('renamed',{activity:{...entry('renamed').activity,name:'合同_初稿.docx'},resource:current});
  mocks.timeline.mockResolvedValue(page([historical]));
  const user=userEvent.setup();const {rerender}=render(<ActivityTimeline {...props}/>);
  const article=await screen.findByRole('article');
  expect(within(article).getByText('合同_初稿.docx',{selector:'strong'})).toHaveAttribute('title','合同_初稿.docx');
  expect(within(article).queryByText('合同_定稿.docx',{selector:'strong'})).not.toBeInTheDocument();
  expect(screen.queryByRole('button',{name:'打开 合同_初稿.docx'})).not.toBeInTheDocument();
  await user.click(within(article).getByRole('button',{name:'打开 合同_定稿.docx'}));
  await user.click(within(article).getByRole('button',{name:'定位 合同_定稿.docx'}));
  expect(props.onOpen).toHaveBeenCalledExactlyOnceWith(current);
  expect(props.onReveal).toHaveBeenCalledExactlyOnceWith(current);
  rerender(<ActivityTimeline {...props} lang="en"/>);
  expect(within(article).getByText('合同_初稿.docx',{selector:'strong'})).toBeVisible();
  await user.click(within(article).getByRole('button',{name:'Open 合同_定稿.docx'}));
  await user.click(within(article).getByRole('button',{name:'Reveal 合同_定稿.docx'}));
  await user.click(screen.getByRole('button',{name:'合同_定稿.docx'}));
  expect(props.onOpen).toHaveBeenCalledTimes(3);expect(props.onOpen).toHaveBeenLastCalledWith(current);
  expect(props.onReveal).toHaveBeenCalledTimes(2);expect(props.onReveal).toHaveBeenLastCalledWith(current);
  expect(mocks.timeline).toHaveBeenCalledOnce();
});

it('旧记录空历史名回退当前资源名，且无资源的应用记录保留后端名称不增加打开动作',async()=>{
  const current={...file,name:'旧库资料_当前名.docx'};
  const legacy=entry('legacy',{activity:{...entry('legacy').activity,name:''},resource:current});
  const app=entry('deleted-workspace',{activity:{...entry('deleted-workspace').activity,name:'已结束的项目',resource_id:null,kind:'workspace_deleted',source:'app'},resource:null});
  mocks.timeline.mockResolvedValue(page([legacy,app]));
  const {rerender}=render(<ActivityTimeline {...props}/>);
  const articles=await screen.findAllByRole('article');
  expect(within(articles[0]).getByText(current.name,{selector:'strong'})).toHaveAttribute('title',current.name);
  expect(within(articles[0]).getByRole('button',{name:'打开 '+current.name})).toBeEnabled();
  expect(within(articles[1]).getByText('已结束的项目',{selector:'strong'})).toBeVisible();
  expect(within(articles[1]).queryByRole('button')).not.toBeInTheDocument();
  rerender(<ActivityTimeline {...props} lang="en"/>);
  expect(within(articles[0]).getByText(current.name,{selector:'strong'})).toBeVisible();
  expect(within(articles[0]).getByRole('button',{name:'Open '+current.name})).toBeEnabled();
  expect(within(articles[1]).getByText('已结束的项目',{selector:'strong'})).toBeVisible();
  expect(mocks.timeline).toHaveBeenCalledOnce();
});

it('身份待确认资源保留已授权的历史标题但不会打开或定位未知对象',async()=>{
  const current={...file,name:'需确认的当前引用.docx',status:'identity_unconfirmed'};
  mocks.timeline.mockResolvedValue(page([entry('unconfirmed',{activity:{...entry('unconfirmed').activity,name:'先前观察的资料.docx'},resource:current})]));
  const user=userEvent.setup();const {rerender}=render(<ActivityTimeline {...props}/>);
  const article=await screen.findByRole('article');
  expect(within(article).getByText('先前观察的资料.docx',{selector:'strong'})).toBeVisible();
  expect(within(article).getByText('身份待确认')).toBeVisible();
  const open=within(article).getByRole('button',{name:'打开 '+current.name});
  const reveal=within(article).getByRole('button',{name:'定位 '+current.name});
  expect(open).toBeDisabled();expect(reveal).toBeDisabled();
  await user.click(open);await user.click(reveal);
  rerender(<ActivityTimeline {...props} lang="en"/>);
  expect(within(article).getByText('Identity unconfirmed')).toBeVisible();
  expect(within(article).getByRole('button',{name:'Open '+current.name})).toBeDisabled();
  expect(within(article).getByRole('button',{name:'Reveal '+current.name})).toBeDisabled();
  expect(screen.queryByText(current.location)).not.toBeInTheDocument();
  expect(props.onOpen).not.toHaveBeenCalled();expect(props.onReveal).not.toHaveBeenCalled();
});

it('撤权由API移除记录后旧历史名与当前资源名同步消失，不从旧快照补回标题或入口',async()=>{
  const current={...file,name:'撤权前当前名.docx'};
  const historical=entry('revoked',{activity:{...entry('revoked').activity,name:'撤权前历史名.docx'},resource:current});
  mocks.timeline.mockResolvedValueOnce(page([historical],{calendar:[{at,count:1}]}));
  const {rerender}=render(<ActivityTimeline {...props}/>);
  await screen.findByText('撤权前历史名.docx',{selector:'strong'});
  mocks.overview.mockResolvedValue({...metadata,revision:2,permission_version:2});
  mocks.timeline.mockResolvedValue(page([],{revision:2,permission_version:2}));
  rerender(<ActivityTimeline {...props} revision={2}/>);
  expect(screen.queryByRole('article')).not.toBeInTheDocument();
  expect(screen.queryByText('撤权前历史名.docx')).not.toBeInTheDocument();
  expect(screen.queryByText('撤权前当前名.docx')).not.toBeInTheDocument();
  await screen.findByText('这个月暂无活动记录');
  expect(screen.getByText('本月 0 条记录')).toBeVisible();
  expect(screen.queryByRole('button',{name:/撤权前/})).not.toBeInTheDocument();
  rerender(<ActivityTimeline {...props} revision={2} lang="en"/>);
  expect(screen.getByText('No activity for this month')).toBeVisible();
  expect(screen.queryByText(/撤权前/)).not.toBeInTheDocument();
  expect(props.onOpen).not.toHaveBeenCalled();expect(props.onReveal).not.toHaveBeenCalled();
});

it('普通版本刷新和手动刷新期间保留时间线卡片与日历，强失效仍立即隐藏',async()=>{
  mocks.timeline.mockResolvedValue(page([entry('stable')]));
  const user=userEvent.setup();const {rerender}=render(<ActivityTimeline {...props}/>);
  const article=await screen.findByRole('article');
  let finish!:(value:TimelinePage)=>void;
  mocks.timeline.mockImplementationOnce(()=>new Promise(resolve=>{finish=resolve;}));
  rerender(<ActivityTimeline {...props} refreshRevision={1}/>);
  await waitFor(()=>expect(finish).toBeTypeOf('function'));
  expect(screen.getByRole('article')).toBe(article);
  expect(screen.queryByRole('heading',{name:'正在读取活动'})).not.toBeInTheDocument();
  await act(async()=>finish(page([entry('stable')])));
  await waitFor(()=>expect(screen.getByRole('button',{name:'刷新时间线'})).toBeEnabled());
  mocks.timeline.mockImplementationOnce(()=>new Promise(resolve=>{finish=resolve;}));
  await user.click(screen.getByRole('button',{name:'刷新时间线'}));
  expect(screen.getByRole('article')).toBe(article);
  await act(async()=>finish(page([entry('stable')])));
  mocks.overview.mockImplementationOnce(()=>new Promise(()=>{}));
  rerender(<ActivityTimeline {...props} revision={2} refreshRevision={1}/>);
  expect(screen.queryByRole('article')).not.toBeInTheDocument();
});
