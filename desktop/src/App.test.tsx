import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { Overview, Resource, ResourcePage, SearchCursor, SearchResponse } from './contracts';

const mocks = vi.hoisted(() => ({ notes:vi.fn(),writeNote:vi.fn(), documentTracking:vi.fn(), setDocumentTracking:vi.fn(), overview: vi.fn(), resources: vi.fn(), search: vi.fn(), mutate: vi.fn(), open: vi.fn(), searchPage: vi.fn(), importDrop: vi.fn(), pickFiles: vi.fn(), relink:vi.fn(), backup:vi.fn(), restore:vi.fn(), webMetadata: vi.fn(), timeline:vi.fn(), cancelDrop:vi.fn(), cancelSearch:vi.fn(), requestCounter:0, pickRoot:vi.fn(), rescan:vi.fn(), listen: vi.fn() }));
vi.mock('./api', async importOriginal => ({ ...(await importOriginal<typeof import('./api')>()), native: true, api: mocks, nextSearchRequestId:()=>++mocks.requestCounter }));
vi.mock('@tauri-apps/api/event', () => ({ listen: mocks.listen }));
import App from './App';

const meta: Overview = { session: 's', permission_version: 1, revision: 1, sqlite_version: 'test', roots: [], file_grants: [], activities: [], workspaces: [{ id: 'workspace1', name: '客户 A', count: 0 }] };
const page: ResourcePage = { session: 's', permission_version: 1, revision: 1, has_more: false, items: [] };
const file = (id: string, name: string): Resource => ({ id, name, kind: 'file', location: '/fixture/' + name, status: 'available', modified_at: 1, size: 1, favorite: false, workspace_ids: [] });
const cursor = (offset:number,query='合同'):SearchCursor => ({version:4,session:'s',permission_version:1,revision:1,next_offset:offset,rank_position:{rank:1,name:query+'_099',id:'fixture-position'},criteria:{query,workspace_id:null,kind:'all',sort:'relevance',modified_from:null,modified_to:null,path_prefix:null}});

beforeEach(() => {
  vi.resetAllMocks();mocks.notes.mockResolvedValue({scope:"s:0",items:[]});vi.spyOn(window,'scrollTo').mockImplementation(()=>{});mocks.requestCounter=0;mocks.cancelSearch.mockResolvedValue(true);
  mocks.documentTracking.mockResolvedValue({supported:false,roots:[],files:[]});mocks.setDocumentTracking.mockResolvedValue(undefined);
  mocks.pickFiles.mockResolvedValue(null);mocks.relink.mockResolvedValue(false);mocks.backup.mockResolvedValue(false);mocks.restore.mockResolvedValue(false);
  mocks.webMetadata.mockResolvedValue({title:'招聘后台',final_url:'https://example.com/jobs'}); mocks.overview.mockResolvedValue(structuredClone(meta)); mocks.resources.mockResolvedValue(page);
  mocks.listen.mockResolvedValue(() => {});mocks.cancelDrop.mockResolvedValue(true);mocks.pickRoot.mockResolvedValue(null);mocks.timeline.mockResolvedValue({session:'s',permission_version:1,revision:1,events:[],calendar:[],has_more:false}); mocks.mutate.mockResolvedValue({ id: 'new', changed: 1 });
});

describe('工作台交互回归', () => {
  it('在工作区添加网页时将目标交给同一个后端命令', async () => {
    const user = userEvent.setup(); render(<App initialView="home" />);
    await user.click(await within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).findByRole('button', { name: /^客户 A/ }));
    await user.click(screen.getAllByRole('button', { name: '添加网页' })[0]);
    const dialog = screen.getByRole('dialog');
    await user.type(within(dialog).getByRole('textbox', { name: '标题，可修改' }), '招聘后台');
    await user.type(within(dialog).getByRole('textbox', { name: '网址' }), 'https://example.com/jobs');
    await user.click(within(dialog).getByRole('button', { name: '保存' }));
    expect(mocks.mutate).toHaveBeenCalledExactlyOnceWith({ type: 'save_web', id: null, name: '招聘后台', url: 'https://example.com/jobs', workspace_id: 'workspace1' });
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  });
  it('后端保存失败时弹窗内显示错误且保留输入', async () => {
    mocks.mutate.mockRejectedValue({ message: '本地数据操作失败' });
    const user = userEvent.setup(); render(<App initialView="home" />);
    await user.click((await screen.findAllByRole('button', { name: '新建工作区' }))[1]);
    const dialog = screen.getByRole('dialog');
    await user.type(within(dialog).getByRole('textbox', { name: '名称' }), '保留的输入');
    await user.click(within(dialog).getByRole('button', { name: '保存' }));
    expect(await within(dialog).findByRole('alert')).toHaveTextContent('本地数据操作失败');
    expect(within(dialog).getByRole('textbox')).toHaveValue('保留的输入');
  });
  it('删除失败时保留当前工作区，取消确认不发出删除命令', async () => {
    mocks.mutate.mockRejectedValue({ message: '删除失败' });
    const user = userEvent.setup(); render(<App initialView="home" />);
    await user.click(await within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).findByRole('button', { name: /^客户 A/ }));
    await user.click(screen.getByRole('button', { name: '删除工作区' }));
    await user.click(within(screen.getByRole('dialog')).getByRole('button', { name: '取消' }));
    expect(mocks.mutate).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: '删除工作区' }));
    const dialog = screen.getByRole('dialog');
    await user.click(within(dialog).getByRole('button', { name: '删除工作区' }));
    await within(dialog).findByRole('alert');
    expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('客户 A');
  });
  it('快速输入后，较早请求晚返回不能覆盖当前搜索结果', async () => {
    const pending: Array<{ request: { request_id: number; query: string }; resolve: (value: unknown) => void }> = [];
    mocks.search.mockImplementation(request => new Promise(resolve => pending.push({ request, resolve })));
    const user = userEvent.setup(); render(<App initialView="home" />);
    await within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).findByRole('button', { name: /^客户 A/ });
    const search = screen.getByRole('combobox');
    await user.type(search, '张'); await waitFor(() => expect(pending).toHaveLength(1));
    await user.type(search, '三'); await waitFor(() => expect(pending).toHaveLength(2));
    await act(async () => pending[1].resolve({ request_id: pending[1].request.request_id, page: { ...page, items: [file('new', '张三合同')] } }));
    await screen.findByRole('option', { name: /张三合同/ });
    await act(async () => pending[0].resolve({ request_id: pending[0].request.request_id, page: { ...page, items: [file('old', '过期结果')] } }));
    expect(screen.queryByRole('option', { name: /过期结果/ })).not.toBeInTheDocument();
    expect(screen.getByRole('option', { name: /张三合同/ })).toBeVisible();
    await user.keyboard('{ArrowDown}{Enter}');
    expect(mocks.open).toHaveBeenCalledExactlyOnceWith('new');
  });
  it('英文设置不向用户显示内部扫描状态码', async () => {
    mocks.overview.mockResolvedValue({ ...meta, roots: [{ id: 'root', path: '/fixture', active: true, paused: false, scan_state: 'completed', location_status: 'available', scanned: 2, failures: 0, exclusions: [] }] });
    const user = userEvent.setup(); render(<App initialView="home" />);
    await within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).findByRole('button', { name: /^客户 A/ });
    await user.click(screen.getByRole('button', { name: 'EN' }));
    await user.click(screen.getByRole('button', { name: 'Locations & settings' }));
    expect(await screen.findByText(/Scan complete/)).toBeVisible();
    expect(screen.queryByText(/completed/)).not.toBeInTheDocument();
  });
  it('自动标题晚返回不能覆盖用户手动修改的标题', async()=>{
    let resolve!:(value:unknown)=>void;
    mocks.webMetadata.mockImplementation(()=>new Promise(done=>{resolve=done;}));
    const user=userEvent.setup();render(<App initialView="home"/>);
    await user.click(await screen.findByRole('button',{name:'添加网页'}));
    const dialog=screen.getByRole('dialog');
    await user.type(within(dialog).getByRole('textbox',{name:'网址'}),'https://example.com');
    await waitFor(()=>expect(mocks.webMetadata).toHaveBeenCalledWith('https://example.com'),{timeout:2000});
    const title=within(dialog).getByRole('textbox',{name:'标题，可修改'});
    await user.clear(title);await user.type(title,'我自己的标题');
    await act(async()=>resolve({title:'自动获取的标题',final_url:'https://example.com'}));
    expect(title).toHaveValue('我自己的标题');
  });
  it('退出添加搜索后快捷键进入普通搜索，回车打开而不继续加入',async()=>{
    mocks.search.mockImplementation(request=>Promise.resolve({request_id:request.request_id,page:{...page,items:[file('f','合同.docx')]}}));
    const user=userEvent.setup();render(<App initialView="home"/>);
    await user.click(await within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).findByRole('button',{name:/^客户 A/}));
    await user.click(screen.getAllByRole('button',{name:'添加文件'})[0]);
    await user.click(screen.getByRole('heading',{level:1}));
    await user.keyboard('{Control>}k{/Control}');
    await user.type(screen.getByRole('combobox'),'合同');await screen.findByRole('option');
    await user.keyboard('{ArrowDown}{Enter}');
    expect(mocks.open).toHaveBeenCalledExactlyOnceWith('f');expect(mocks.mutate).not.toHaveBeenCalled();
  });
  it('全部结果入口使用分页查询并允许选择类型和排序',async()=>{
    mocks.search.mockImplementation(request=>Promise.resolve({request_id:request.request_id,page:{...page,items:[file('f','合同.docx')]}}));
    mocks.searchPage.mockImplementation(request=>Promise.resolve({request_id:request.request.request_id,page:{...page,items:[file('f','合同.docx')]}}));
    const user=userEvent.setup();render(<App initialView="home"/>);
    await within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).findByRole('button',{name:/^客户 A/});
    await user.type(screen.getByRole('combobox'),'合同');await screen.findByRole('option');
    vi.mocked(window.scrollTo).mockClear();
    await user.click(screen.getByRole('button',{name:'查看全部搜索结果'}));
    await screen.findByRole('heading',{name:'搜索结果：合同'});
    expect(window.scrollTo).toHaveBeenCalledExactlyOnceWith(0,0);
    expect(screen.getByRole('combobox',{name:'全局搜索'})).toHaveValue('合同');
    vi.mocked(window.scrollTo).mockClear();
    await user.selectOptions(screen.getByRole('combobox',{name:'搜索排序'}),'modified');
    await waitFor(()=>expect(mocks.searchPage).toHaveBeenLastCalledWith(expect.objectContaining({sort:'modified'})));
    expect(window.scrollTo).not.toHaveBeenCalled();
  });

});

it('关闭标题获取后可直接保存裸域名，不依赖网络',async()=>{
  const user=userEvent.setup();render(<App initialView="home"/>);
  await user.click(await screen.findByRole('button',{name:'添加网页'}));
  const dialog=screen.getByRole('dialog');
  await user.click(within(dialog).getByRole('checkbox',{name:'自动获取网站标题'}));
  await user.type(within(dialog).getByRole('textbox',{name:'网址'}),'example.com/jobs');
  await user.click(within(dialog).getByRole('button',{name:'保存'}));
  expect(mocks.webMetadata).not.toHaveBeenCalled();
  expect(mocks.mutate).toHaveBeenCalledWith({type:'save_web',id:null,name:'example.com',url:'https://example.com/jobs',workspace_id:null});
});

it('网址失效后清除获取状态，之前的回包不能填写标题',async()=>{
  let resolve!:(value:unknown)=>void;
  mocks.webMetadata.mockImplementation(()=>new Promise(done=>{resolve=done;}));
  const user=userEvent.setup();render(<App initialView="home"/>);
  await user.click(await screen.findByRole('button',{name:'添加网页'}));
  const dialog=screen.getByRole('dialog');const url=within(dialog).getByRole('textbox',{name:'网址'});
  await user.type(url,'https://example.com');await waitFor(()=>expect(mocks.webMetadata).toHaveBeenCalled(),{timeout:2000});
  await user.clear(url);await user.type(url,'javascript:alert(1)');
  expect(within(dialog).queryByText(/正在获取标题/)).not.toBeInTheDocument();
  await act(async()=>resolve({title:'过期标题',final_url:'https://example.com'}));
  expect(within(dialog).getByRole('textbox',{name:'标题，可修改'})).not.toHaveValue('过期标题');
});

it('批量转移逐项提交，失败项保留来源并展示结果',async()=>{
  mocks.overview.mockResolvedValue({...meta,workspaces:[...meta.workspaces,{id:'workspace2',name:'客户 B',count:0}]});
  mocks.resources.mockResolvedValue({...page,items:[file('a','成功资料.txt'),file('b','失效资料.txt')]});
  mocks.mutate.mockImplementation(mutation=>mutation.resource_ids?.[0]==='b'?Promise.reject({message:'原件未授权',message_en:'The original file is not authorized.'}):Promise.resolve({changed:1,id:null}));
  const user=userEvent.setup();render(<App initialView="home"/>);
  await user.click(await within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).findByRole('button',{name:/^客户 A/}));
  await user.click(await screen.findByRole('checkbox',{name:'选择 成功资料.txt'}));
  await user.click(screen.getByRole('checkbox',{name:'选择 失效资料.txt'}));
  await user.click(screen.getByRole('button',{name:'移到工作区'}));
  const dialog=screen.getByRole('dialog');await user.selectOptions(within(dialog).getByRole('combobox'),'workspace2');
  await user.click(within(dialog).getByRole('button',{name:'保存'}));
  expect(await screen.findByText('1 项成功 · 1 项失败')).toBeVisible();
  expect(screen.getByText('失败项保留原有引用。')).toBeVisible();
  expect(mocks.mutate).toHaveBeenNthCalledWith(1,{type:'transfer',source:'workspace1',target:'workspace2',resource_ids:['a']});
  expect(mocks.mutate).toHaveBeenNthCalledWith(2,{type:'transfer',source:'workspace1',target:'workspace2',resource_ids:['b']});
  await user.click(screen.getByRole('button',{name:'EN'}));
  expect(screen.getByText('失效资料.txt',{selector:'.batch-report b'}).closest('li')).toHaveTextContent('失效资料.txt · The original file is not authorized.');
  expect(mocks.mutate).toHaveBeenCalledTimes(2);
});

it('完整搜索修改关键词回到第一页，不沿用旧的分页偏移',async()=>{
  mocks.search.mockImplementation(request=>Promise.resolve({request_id:request.request_id,page:{...page,items:[file('a','合同')]}}));
  mocks.searchPage.mockImplementation(request=>Promise.resolve({request_id:request.request.request_id,page:{...page,has_more:true,items:[file(String(request.offset),'合同'+request.offset)]},next_cursor:cursor(request.offset+100,request.request.query)}));
  const user=userEvent.setup();render(<App initialView="home"/>);
  await within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).findByRole('button',{name:/^客户 A/});
  await user.type(screen.getByRole('combobox'),'合同');await screen.findByRole('option');await user.click(screen.getByRole('button',{name:'查看全部搜索结果'}));
  await user.click(await screen.findByRole('button',{name:'加载更多'}));
  await waitFor(()=>expect(mocks.searchPage).toHaveBeenLastCalledWith(expect.objectContaining({offset:100})));
  await user.type(screen.getByRole('textbox',{name:'完整搜索词'}),'新');
  await waitFor(()=>expect(mocks.searchPage).toHaveBeenLastCalledWith(expect.objectContaining({offset:0,request:expect.objectContaining({query:'合同新'})})));
  expect(mocks.searchPage.mock.calls.at(-1)?.[0].cursor).toBeUndefined();
});

it('没有搜索匹配时不误导为正在扫描或需要重新授权',async()=>{
  mocks.searchPage.mockImplementation(request=>Promise.resolve({request_id:request.request.request_id,page}));
  render(<App initialView="search"/>);fireEvent.change(screen.getByRole("textbox",{name:"完整搜索词"}),{target:{value:"合同"}});
  expect(await screen.findByRole('heading',{name:'没有匹配的资料'})).toBeVisible();
  expect(screen.queryByText(/扫描期间资料会逐步出现/)).not.toBeInTheDocument();
});

it('动态页空库可直接添加管理位置，取消不制造文件',async()=>{
  const user=userEvent.setup();render(<App initialView="activity"/>);
  const button=await screen.findByRole('button',{name:'添加管理位置'});await user.click(button);
  expect(mocks.pickRoot).toHaveBeenCalledOnce();
  expect(mocks.mutate).not.toHaveBeenCalled();
});

it('混合原生拖入展示成功及未授权，完成后丢弃短时凭据',async()=>{
  mocks.importDrop.mockResolvedValue({items:[{path:'/fixture/a.txt',status:'added',resource_id:'a',message:null},{path:'/fixture/b.txt',status:'not_authorized',resource_id:null,message:'未授权'}]});
  const user=userEvent.setup();render(<App initialView="home"/>);
  await within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).findByRole('button',{name:/^客户 A/});
  const handler=mocks.listen.mock.calls.find(([name])=>name==='files-dropped')![1];
  Object.defineProperty(document,'elementFromPoint',{configurable:true,value:()=>document.querySelector('main')});
  const start=mocks.listen.mock.calls.find(([name])=>name==='files-drop-started')![1];
  await act(async()=>{start({payload:{token:'drop-token',x:100,y:100,count:2}});handler({payload:{token:'drop-token',x:100,y:100,count:2}});});
  expect(mocks.importDrop).toHaveBeenCalledWith('drop-token',null,false,'zh');
  expect(await screen.findByText('1 项已加入 · 0 项已存在 · 1 项未授权 · 0 项失败')).toBeVisible();
  expect(screen.getByRole('button',{name:'仅授权未加入的文件'})).toBeVisible();
  await user.click(screen.getByRole('button',{name:'完成'}));expect(mocks.cancelDrop).toHaveBeenCalledWith('drop-token');
});

it('目录变更通知后迟到的旧资源读取不能恢复被隐藏名称',async()=>{
  let resolve!:(value:ResourcePage)=>void;
  mocks.resources.mockImplementationOnce(()=>new Promise(done=>{resolve=done;})).mockResolvedValue(page);
  render(<App initialView="home"/>);
  await waitFor(()=>expect(mocks.resources).toHaveBeenCalledOnce());
  const handler=mocks.listen.mock.calls.find(([name])=>name==='catalog-changed')![1];
  await act(async()=>{handler({payload:null});resolve({...page,items:[file('old','已撤权的旧文件')]});});
  await waitFor(()=>expect(mocks.resources).toHaveBeenCalledTimes(2));
  expect(screen.queryByRole('button',{name:/已撤权的旧文件/})).not.toBeInTheDocument();
});

it('日期与路径筛选传本地半开日期并清除回到第一页',async()=>{
  mocks.searchPage.mockImplementation(request=>Promise.resolve({request_id:request.request.request_id,page}));
  const user=userEvent.setup();render(<App initialView="search"/>);fireEvent.change(screen.getByRole("textbox",{name:"完整搜索词"}),{target:{value:"合同"}});
  await screen.findByRole('heading',{name:'没有匹配的资料'});
  fireEvent.change(screen.getByLabelText('修改日期，从'),{target:{value:'2026-03-08'}});
  fireEvent.change(screen.getByLabelText('至'),{target:{value:'2026-03-08'}});
  await user.type(screen.getByRole('textbox',{name:'路径或网址前缀'}),'/资料/%_');
  await waitFor(()=>expect(mocks.searchPage).toHaveBeenLastCalledWith(expect.objectContaining({offset:0,modified_from:new Date(2026,2,8).getTime(),modified_to:new Date(2026,2,9).getTime(),path_prefix:'/资料/%_'})));
  await user.click(screen.getByRole('button',{name:'清除筛选'}));
  await waitFor(()=>expect(mocks.searchPage).toHaveBeenLastCalledWith(expect.objectContaining({offset:0,modified_from:undefined,modified_to:undefined,path_prefix:undefined})));
});

it('工作区范围完整搜索保留范围，全局添加网页不继承隐藏工作区',async()=>{
  mocks.search.mockImplementation(request=>Promise.resolve({request_id:request.request_id,page:{...page,items:[file('a','合同')]}}));
  mocks.searchPage.mockImplementation(request=>Promise.resolve({request_id:request.request.request_id,page}));
  const user=userEvent.setup();render(<App initialView="home"/>);
  await user.click(await within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).findByRole('button',{name:/^客户 A/}));
  await user.type(screen.getByRole('combobox'),'合同');await screen.findByRole('option');
  await user.click(screen.getByRole('button',{name:'当前工作区'}));await screen.findByRole('option');
  await user.click(screen.getByRole('button',{name:'查看全部搜索结果'}));
  await waitFor(()=>expect(mocks.searchPage).toHaveBeenLastCalledWith(expect.objectContaining({request:expect.objectContaining({workspace_id:'workspace1'})})));
  await user.click(screen.getByRole('button',{name:'添加网页'}));const dialog=screen.getByRole('dialog');
  await user.click(within(dialog).getByRole('checkbox',{name:'自动获取网站标题'}));
  await user.type(within(dialog).getByRole('textbox',{name:'网址'}),'example.com');
  await user.click(within(dialog).getByRole('button',{name:'保存'}));
  expect(mocks.mutate).toHaveBeenCalledWith(expect.objectContaining({type:'save_web',workspace_id:null}));
});

it('慢盘拖入开始后切换工作区，完成时仍关联松手时的工作区',async()=>{
  mocks.overview.mockResolvedValue({...meta,workspaces:[...meta.workspaces,{id:'workspace2',name:'客户 B',count:0}]});
  mocks.importDrop.mockResolvedValue({items:[]});const user=userEvent.setup();render(<App initialView="home"/>);
  const sidebar=within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/}));
  await user.click(await sidebar.findByRole('button',{name:/^客户 A/}));
  const start=mocks.listen.mock.calls.find(([name])=>name==='files-drop-started')![1];
  const ready=mocks.listen.mock.calls.find(([name])=>name==='files-dropped')![1];
  Object.defineProperty(document,'elementFromPoint',{configurable:true,value:()=>document.querySelector('main')});
  await act(async()=>start({payload:{token:'slow-drop',x:100,y:100,count:1}}));
  expect(screen.getByText('正在确认拖入的文件…')).toBeVisible();
  await user.click(sidebar.getByRole('button',{name:/^客户 B/}));await screen.findByRole('heading',{name:'客户 B'});
  await act(async()=>ready({payload:{token:'slow-drop',x:100,y:100,count:1}}));
  expect(mocks.importDrop).toHaveBeenCalledExactlyOnceWith('slow-drop','workspace1',false,'zh');
});

it('旧或没有开始凭据的ready被忽略，新拖入只消费自己的目标',async()=>{
  mocks.overview.mockResolvedValue({...meta,workspaces:[...meta.workspaces,{id:'workspace2',name:'客户 B',count:0}]});
  mocks.importDrop.mockResolvedValue({items:[]});const user=userEvent.setup();render(<App initialView="home"/>);
  const sidebar=within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/}));await user.click(await sidebar.findByRole('button',{name:/^客户 A/}));
  const start=mocks.listen.mock.calls.find(([name])=>name==='files-drop-started')![1];const ready=mocks.listen.mock.calls.find(([name])=>name==='files-dropped')![1];
  Object.defineProperty(document,'elementFromPoint',{configurable:true,value:()=>document.querySelector('main')});
  const payload=(token:string)=>({payload:{token,x:100,y:100,count:1}});
  await act(async()=>ready(payload('forged')));expect(mocks.importDrop).not.toHaveBeenCalled();
  await act(async()=>start(payload('old')));await user.click(sidebar.getByRole('button',{name:/^客户 B/}));await screen.findByRole('heading',{name:'客户 B'});
  await act(async()=>start(payload('new')));await act(async()=>ready(payload('old')));expect(mocks.importDrop).not.toHaveBeenCalled();
  await act(async()=>ready(payload('new')));expect(mocks.importDrop).toHaveBeenCalledExactlyOnceWith('new','workspace2',false,'zh');
});

it('准备阶段取消拖入，迟到ready不能授权或关联',async()=>{
  const user=userEvent.setup();render(<App initialView="home"/>);await within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).findByRole('button',{name:/^客户 A/});
  const start=mocks.listen.mock.calls.find(([name])=>name==='files-drop-started')![1];const ready=mocks.listen.mock.calls.find(([name])=>name==='files-dropped')![1];
  Object.defineProperty(document,'elementFromPoint',{configurable:true,value:()=>document.querySelector('main')});const event={payload:{token:'cancelled',x:100,y:100,count:1}};
  await act(async()=>start(event));await user.click(screen.getByRole('button',{name:'取消拖入'}));
  await act(async()=>ready(event));expect(mocks.cancelDrop).toHaveBeenCalledWith('cancelled');expect(mocks.importDrop).not.toHaveBeenCalled();
});

it('删改和Esc关闭搜索时取消对应快速查询，迟到结果不再显示',async()=>{
  mocks.search.mockImplementation(()=>new Promise(()=>{}));const user=userEvent.setup();render(<App initialView="home"/>);
  await within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).findByRole('button',{name:/^客户 A/});const input=screen.getByRole('combobox');
  await user.type(input,'合同');await waitFor(()=>expect(mocks.search).toHaveBeenCalledOnce());
  const first=mocks.search.mock.calls[0][0].request_id;
  await user.type(input,'新');await waitFor(()=>expect(mocks.cancelSearch).toHaveBeenCalledWith('quick',first));await waitFor(()=>expect(mocks.search).toHaveBeenCalledTimes(2));
  const second=mocks.search.mock.calls[1][0].request_id;expect(second).toBeGreaterThan(first);
  await user.keyboard('{Escape}');await waitFor(()=>expect(mocks.cancelSearch).toHaveBeenCalledWith('quick',second));expect(screen.queryByRole('listbox')).not.toBeInTheDocument();
});
it('完整搜索改筛选或离开页面会取消其独立page查询',async()=>{
  mocks.searchPage.mockImplementation(()=>new Promise(()=>{}));const user=userEvent.setup();render(<App initialView="search"/>);fireEvent.change(screen.getByRole("textbox",{name:"完整搜索词"}),{target:{value:"合同"}});
  await waitFor(()=>expect(mocks.searchPage).toHaveBeenCalledOnce());const first=mocks.searchPage.mock.calls[0][0].request.request_id;
  await user.selectOptions(screen.getByRole('combobox',{name:'搜索排序'}),'modified');await waitFor(()=>expect(mocks.cancelSearch).toHaveBeenCalledWith('page',first));
  await waitFor(()=>expect(mocks.searchPage).toHaveBeenCalledTimes(2));const second=mocks.searchPage.mock.calls[1][0].request.request_id;
  await user.click(screen.getByRole('button',{name:'个人工作台'}));await waitFor(()=>expect(mocks.cancelSearch).toHaveBeenCalledWith('page',second));
});

it('跳到网页分类后回车打开当前网页，不打开滚出视口的第一项',async()=>{
  const web={...file('web','合同网页'),kind:'web',location:'https://example.com/'};
  mocks.search.mockImplementation(request=>Promise.resolve({request_id:request.request_id,page:{...page,items:[file('a','合同文件'),web]}}));
  const user=userEvent.setup();render(<App initialView="home"/>);await within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).findByRole('button',{name:/^客户 A/});
  await user.type(screen.getByRole('combobox'),'合同');await screen.findByRole('option',{name:/合同网页/});
  await user.click(screen.getByRole('button',{name:'跳到网页结果'}));await user.keyboard('{Enter}');
  expect(mocks.open).toHaveBeenCalledExactlyOnceWith('web');
});

it('完整搜索原样回传后端游标，失效后清空旧页并从首页重读一次',async()=>{
  const issued=cursor(100,'');
  mocks.searchPage.mockImplementationOnce(request=>Promise.resolve({request_id:request.request.request_id,page:{...page,has_more:true,items:[file('old','变动前资料')]},next_cursor:issued}))
    .mockRejectedValueOnce({code:'stale_cursor',message:'结果已变更'})
    .mockImplementationOnce(request=>Promise.resolve({request_id:request.request.request_id,page:{...page,revision:2,items:[file('new','变动后资料')]}}));
  const user=userEvent.setup();render(<App initialView="search"/>);fireEvent.change(screen.getByRole("textbox",{name:"完整搜索词"}),{target:{value:"合同"}});
  await user.click(await screen.findByRole('checkbox',{name:'选择 变动前资料'}));
  await user.click(screen.getByRole('button',{name:'加载更多'}));
  await screen.findByRole('checkbox',{name:'选择 变动后资料'});
  expect(mocks.searchPage.mock.calls.map(([request])=>request.offset)).toEqual([0,100,0]);
  expect(mocks.searchPage.mock.calls[1][0].cursor).toEqual(issued);
  expect(mocks.searchPage.mock.calls[2][0].cursor).toBeUndefined();
  expect(screen.queryByRole('checkbox',{name:'选择 变动前资料'})).not.toBeInTheDocument();
  expect(screen.getByRole('checkbox',{name:'选择 变动后资料'})).not.toBeChecked();
  expect(screen.getByText('资料已更新，正在重新加载搜索结果')).toBeVisible();
});

it('首页报错不循环自动重读，缺少后续游标也不发任意offset请求',async()=>{
  mocks.searchPage.mockRejectedValue({code:'stale_cursor',message:'首页无法读取'});
  const {unmount}=render(<App initialView="search"/>);fireEvent.change(screen.getByRole("textbox",{name:"完整搜索词"}),{target:{value:"合同"}});
  expect(await screen.findByText('首页无法读取')).toBeVisible();
  expect(mocks.searchPage).toHaveBeenCalledOnce();unmount();mocks.searchPage.mockReset();
  mocks.searchPage.mockImplementation(request=>Promise.resolve({request_id:request.request.request_id,page:{...page,has_more:true,items:[file('old','不完整分页')]}}));
  render(<App initialView="search"/>);fireEvent.change(screen.getByRole("textbox",{name:"完整搜索词"}),{target:{value:"合同"}});
  expect(await screen.findByText('搜索分页信息不完整，请重试')).toBeVisible();
  expect(screen.queryByRole('button',{name:'加载更多'})).not.toBeInTheDocument();
  expect(mocks.searchPage).toHaveBeenCalledOnce();
});

it('迟到的旧分页不能覆盖新查询的资料或后续游标',async()=>{
  let resolveOld!:(value:SearchResponse)=>void;
  const oldCursor=cursor(100,'');const freshCursor=cursor(100,'新');
  mocks.search.mockImplementation(request=>Promise.resolve({request_id:request.request_id,page}));
  mocks.searchPage.mockImplementationOnce(request=>Promise.resolve({request_id:request.request.request_id,page:{...page,has_more:true,items:[file('old','旧查询资料')]},next_cursor:oldCursor}))
    .mockImplementationOnce(()=>new Promise(resolve=>{resolveOld=resolve;}))
    .mockImplementationOnce(request=>Promise.resolve({request_id:request.request.request_id,page:{...page,has_more:true,items:[file('new','新查询资料')]},next_cursor:freshCursor}))
    .mockImplementationOnce(request=>Promise.resolve({request_id:request.request.request_id,page:{...page,items:[file('next','新查询后续')]}}));
  const user=userEvent.setup();render(<App initialView="search"/>);fireEvent.change(screen.getByRole("textbox",{name:"完整搜索词"}),{target:{value:"合同"}});
  await user.click(await screen.findByRole('button',{name:'加载更多'}));
  const oldId=mocks.searchPage.mock.calls[1][0].request.request_id;
  await user.type(screen.getByRole('textbox',{name:'完整搜索词'}),'新');
  await screen.findByRole('checkbox',{name:'选择 新查询资料'});
  await act(async()=>resolveOld({request_id:oldId,page:{...page,has_more:true,items:[file('late','过期分页资料')]},next_cursor:cursor(200,'')}));
  expect(screen.queryByRole('checkbox',{name:'选择 过期分页资料'})).not.toBeInTheDocument();
  await user.click(screen.getByRole('button',{name:'加载更多'}));
  await screen.findByRole('checkbox',{name:'选择 新查询后续'});
  expect(mocks.searchPage.mock.calls.at(-1)?.[0].cursor).toEqual(freshCursor);
  expect(mocks.searchPage.mock.calls.map(([request])=>request.offset)).toEqual([0,100,0,100]);
});

it('英文设置中的原生位置、备份和恢复入口传递当前语言',async()=>{
  const user=userEvent.setup();render(<App initialView="home"/>);
  await within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).findByRole('button',{name:/^客户 A/});
  await user.click(screen.getByRole('button',{name:'EN'}));
  await user.click(screen.getByRole('button',{name:'Locations & settings'}));
  await user.click(screen.getByRole('button',{name:'Add location'}));expect(mocks.pickRoot).toHaveBeenCalledExactlyOnceWith('en');
  await user.click(screen.getByRole('button',{name:'Back up'}));expect(mocks.backup).toHaveBeenCalledExactlyOnceWith('en');
  await user.click(screen.getByRole('button',{name:'Restore backup'}));expect(mocks.restore).toHaveBeenCalledExactlyOnceWith('en');
  expect(mocks.mutate).not.toHaveBeenCalled();
});

it('英文资料重新指向和工作区文件选择使用同一当前语言',async()=>{
  mocks.resources.mockResolvedValue({...page,items:[{...file('missing','待定位资料.txt'),status:'missing'}]});
  const user=userEvent.setup();render(<App initialView="home"/>);
  await user.click(await screen.findByRole('button',{name:'EN'}));
  await user.click(await screen.findByRole('button',{name:'Relink'}));expect(mocks.relink).toHaveBeenCalledExactlyOnceWith('missing','en');
  await user.click(await within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).findByRole('button',{name:/^客户 A/}));
  await user.click(await screen.findByRole('button',{name:'Choose local files'}));expect(mocks.pickFiles).toHaveBeenCalledExactlyOnceWith('workspace1','en');
});

it('切换英文后原生拖入和后续授权不沿用监听注册时的语言',async()=>{
  mocks.importDrop.mockResolvedValueOnce({items:[{path:'/fixture/a.txt',status:'not_authorized',resource_id:null,message:null}]}).mockResolvedValueOnce({items:[{path:'/fixture/a.txt',status:'added',resource_id:'a',message:null}]});
  const user=userEvent.setup();render(<App initialView="home"/>);
  await within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).findByRole('button',{name:/^客户 A/});await user.click(screen.getByRole('button',{name:'EN'}));
  Object.defineProperty(document,'elementFromPoint',{configurable:true,value:()=>document.querySelector('main')});
  const start=mocks.listen.mock.calls.find(([name])=>name==='files-drop-started')![1];const ready=mocks.listen.mock.calls.find(([name])=>name==='files-dropped')![1];
  const event={payload:{token:'language-drop',x:100,y:100,count:1}};
  await act(async()=>{start(event);ready(event);});
  expect(mocks.importDrop).toHaveBeenNthCalledWith(1,'language-drop',null,false,'en');
  await user.click(await screen.findByRole('button',{name:'Authorize remaining files'}));
  await waitFor(()=>expect(mocks.importDrop).toHaveBeenNthCalledWith(2,'language-drop',null,true,'en'));
  expect(await screen.findByText('1 added · 0 already present · 0 need authorization · 0 failed')).toBeVisible();
});

it('排除目录失败保留输入，成功只清理当前目录的草稿',async()=>{
  const root={id:'r1',path:'/fixture/One',active:true,paused:false,scan_state:'completed',location_status:'available',scanned:1,failures:0,exclusions:[]};
  mocks.overview.mockResolvedValue({...meta,roots:[root,{...root,id:'r2',path:'/fixture/Two'}]});
  mocks.mutate.mockRejectedValueOnce({message:'排除未保存'}).mockResolvedValueOnce({id:null,changed:1});
  const user=userEvent.setup();render(<App initialView="settings"/>);
  const first=await screen.findByRole('textbox',{name:'排除子目录：/fixture/One'});const second=screen.getByRole('textbox',{name:'排除子目录：/fixture/Two'});
  await user.type(first,'archive/private');await user.type(second,'keep/draft');
  await user.click(screen.getAllByRole('button',{name:'排除子目录'})[0]);
  expect(await screen.findByRole('alert')).toHaveTextContent('排除未保存');expect(first).toHaveValue('archive/private');expect(second).toHaveValue('keep/draft');
  await user.click(screen.getAllByRole('button',{name:'排除子目录'})[0]);
  await waitFor(()=>expect(screen.getByRole('textbox',{name:'排除子目录：/fixture/One'})).toHaveValue(''));
  expect(screen.getByRole('textbox',{name:'排除子目录：/fixture/Two'})).toHaveValue('keep/draft');
  expect(mocks.mutate).toHaveBeenNthCalledWith(2,{type:'exclude',root_id:'r1',relative_path:'archive/private'});
});

it('首次使用常用位置打开对应起点，取消不创建授权或资料，切换语言后使用当前语言',async()=>{
  const user=userEvent.setup();render(<App initialView="activity"/>);
  for(const [label,suggestion] of [['桌面','desktop'],['下载','downloads'],['文档','documents']] as const){
    const group=await screen.findByRole('group',{name:'常用位置'});
    await user.click(within(group).getByRole('button',{name:label}));
    expect(mocks.pickRoot).toHaveBeenLastCalledWith('zh',suggestion);
  }
  await user.click(screen.getByRole('button',{name:'EN'}));
  const group=await screen.findByRole('group',{name:'Common locations'});
  await user.click(within(group).getByRole('button',{name:'Documents'}));
  expect(mocks.pickRoot).toHaveBeenLastCalledWith('en','documents');
  expect(mocks.mutate).not.toHaveBeenCalled();expect(mocks.importDrop).not.toHaveBeenCalled();
  expect(await screen.findByRole('group',{name:'Common locations'})).toBeVisible();
});

it('读取管理范围完成前不显示首次位置建议',async()=>{
  let resolve!:(value:Overview)=>void;mocks.overview.mockImplementation(()=>new Promise(done=>{resolve=done;}));
  render(<App initialView="activity"/>);expect(screen.queryByRole('group',{name:'常用位置'})).not.toBeInTheDocument();
  expect(mocks.pickRoot).not.toHaveBeenCalled();
  await act(async()=>resolve(meta));
  expect(await screen.findByRole('group',{name:'常用位置'})).toBeVisible();
});

it.each(['root','file'] as const)('已有%s授权时不重复显示首次位置建议',async(source)=>{
  mocks.overview.mockResolvedValue({...meta,roots:source==='root'?[{id:'r',path:'/fixture',active:true,paused:false,scan_state:'completed',location_status:'available',scanned:1,failures:0,exclusions:[]}]:[],file_grants:source==='file'?[file('f','资料.txt')]:[]});
  render(<App initialView="settings"/>);
  await waitFor(()=>expect(mocks.overview).toHaveBeenCalled());
  await waitFor(()=>expect(screen.getByRole('heading',{name:'管理位置'})).toBeVisible());
  expect(screen.queryByRole('group',{name:'常用位置'})).not.toBeInTheDocument();expect(mocks.pickRoot).not.toHaveBeenCalled();
});

it('选择器请求期间禁用全部首次位置入口，失败可重试且不添加资料',async()=>{
  let reject!:(error:unknown)=>void;mocks.pickRoot.mockImplementationOnce(()=>new Promise((_,fail)=>{reject=fail;}));
  const user=userEvent.setup();render(<App initialView="activity"/>);
  const group=await screen.findByRole('group',{name:'常用位置'});await user.click(within(group).getByRole('button',{name:'桌面'}));
  for(const button of within(group).getAllByRole('button'))expect(button).toBeDisabled();
  expect(screen.getByRole('button',{name:'添加管理位置'})).toBeDisabled();
  await act(async()=>reject({message:'位置不可访问'}));
  expect(await screen.findByRole('alert')).toHaveTextContent('位置不可访问');
  await user.click(within(group).getByRole('button',{name:'下载'}));
  expect(mocks.pickRoot).toHaveBeenNthCalledWith(2,'zh','downloads');expect(mocks.mutate).not.toHaveBeenCalled();
});

it('明确导航在新页面提交后回顶，进入和再次进入侧栏收藏也回顶',async()=>{
  const titles:Array<string|null>=[];
  mocks.resources.mockResolvedValue({...page,items:[{...file('saved','已收藏合同.txt'),favorite:true}]});
  vi.mocked(window.scrollTo).mockImplementation(()=>{titles.push(document.querySelector('main h1')?.textContent??null);});
  const user=userEvent.setup();render(<App initialView="home"/>);
  await within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).findByRole('button',{name:/^客户 A/});
  vi.mocked(window.scrollTo).mockClear();titles.length=0;
  await user.click(screen.getByRole('button',{name:'管理位置与设置'}));
  expect(window.scrollTo).toHaveBeenCalledExactlyOnceWith(0,0);expect(titles).toEqual(['由你决定管理范围。']);
  await user.click(within(screen.getByRole('navigation')).getByRole('button',{name:'个人工作台'}));
  vi.mocked(window.scrollTo).mockClear();titles.length=0;
  await user.click(within(screen.getByRole('navigation')).getByRole('button',{name:'收藏'}));
  expect(window.scrollTo).toHaveBeenCalledExactlyOnceWith(0,0);expect(titles).toEqual(['收藏']);
  await screen.findByRole('checkbox',{name:'选择 已收藏合同.txt'});
  mocks.resources.mockClear();vi.mocked(window.scrollTo).mockClear();titles.length=0;
  await user.click(within(screen.getByRole('navigation')).getByRole('button',{name:'收藏'}));
  expect(window.scrollTo).toHaveBeenCalledExactlyOnceWith(0,0);expect(titles).toEqual(['收藏']);
  expect(await screen.findByRole('checkbox',{name:'选择 已收藏合同.txt'})).toBeVisible();
  expect(mocks.resources).toHaveBeenCalledExactlyOnceWith({workspace_id:null,kind:'all',mode:'favorites',offset:0});
});

it('同一工作区视图切换UUID也回顶，资料刷新不会再次回顶',async()=>{
  mocks.overview.mockResolvedValue({...meta,workspaces:[...meta.workspaces,{id:'workspace2',name:'客户 B',count:0}]});
  const user=userEvent.setup();render(<App initialView="home"/>);
  const sidebar=screen.getByRole('complementary',{name:/主侧栏|Main sidebar/});
  await user.click(await within(sidebar).findByRole('button',{name:/^客户 A/}));
  vi.mocked(window.scrollTo).mockClear();
  await user.click(within(sidebar).getByRole('button',{name:/^客户 B/}));
  expect(window.scrollTo).toHaveBeenCalledExactlyOnceWith(0,0);
  expect(screen.getByRole('heading',{level:1})).toHaveTextContent('客户 B');
  vi.mocked(window.scrollTo).mockClear();
  const changed=mocks.listen.mock.calls.find(([name])=>name==='catalog-changed')![1];
  await act(async()=>changed({payload:null}));
  await waitFor(()=>expect(mocks.resources).toHaveBeenLastCalledWith(expect.objectContaining({workspace_id:'workspace2'})));
  expect(window.scrollTo).not.toHaveBeenCalled();
});

it('加载更多、页内筛选和语言切换不重置文档滚动',async()=>{
  mocks.resources.mockImplementation(query=>Promise.resolve({...page,has_more:query.offset===0,items:[file(query.offset?'second':'first',query.offset?'第二页资料.txt':'第一页资料.txt')]}));
  const user=userEvent.setup();render(<App initialView="home"/>);
  await screen.findByText('第一页资料.txt');vi.mocked(window.scrollTo).mockClear();
  await user.click(screen.getByRole('button',{name:'加载更多'}));
  await screen.findByText('第二页资料.txt');expect(window.scrollTo).not.toHaveBeenCalled();
  await user.click(within(screen.getByRole('main')).getByRole('button',{name:'最近打开'}));
  await waitFor(()=>expect(mocks.resources).toHaveBeenLastCalledWith(expect.objectContaining({mode:'recent',offset:0})));
  await user.click(screen.getByRole('button',{name:'EN'}));
  expect(window.scrollTo).not.toHaveBeenCalled();
});

it('已显示的保存失败随语言更新，草稿保留且不重复提交',async()=>{
  mocks.mutate.mockRejectedValue({message:'本地数据操作失败',message_en:'The local data operation failed.'});
  const user=userEvent.setup();render(<App initialView="home"/>);
  await user.click((await screen.findAllByRole('button',{name:'新建工作区'}))[1]);
  const dialog=screen.getByRole('dialog');
  await user.type(within(dialog).getByRole('textbox'),'保留的输入');
  await user.click(within(dialog).getByRole('button',{name:'保存'}));
  expect(await within(dialog).findByRole('alert')).toHaveTextContent('本地数据操作失败');
  await user.click(screen.getByRole('button',{name:'EN'}));
  expect(within(dialog).getByRole('alert')).toHaveTextContent('The local data operation failed.');
  expect(screen.getAllByRole('alert')).toHaveLength(2);
  expect(screen.getAllByRole('alert').every(element=>element.textContent?.includes('The local data operation failed.'))).toBe(true);
  expect(within(dialog).getByRole('textbox')).toHaveValue('保留的输入');
  expect(mocks.mutate).toHaveBeenCalledOnce();
});

it('常驻原生错误监听使用当前语言，切语言不重新订阅',async()=>{
  const user=userEvent.setup();render(<App initialView="home"/>);
  await within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).findByRole('button',{name:/^客户 A/});
  const handler=mocks.listen.mock.calls.find(([name])=>name==='operation-error')![1];
  await user.click(screen.getByRole('button',{name:'EN'}));
  await act(async()=>handler({payload:{message:'拖入失败',message_en:'Drop failed.'}}));
  expect(screen.getByRole('alert')).toHaveTextContent('Drop failed.');
  await user.click(screen.getByRole('button',{name:'中文'}));
  expect(screen.getByRole('alert')).toHaveTextContent('拖入失败');
  expect(mocks.listen.mock.calls.filter(([name])=>name==='operation-error')).toHaveLength(1);
});

it.each([null,false,''])('无标准正文的拒绝 %j 仍显示失败并可关闭',async cause=>{
  mocks.pickRoot.mockRejectedValueOnce(cause);
  const user=userEvent.setup();render(<App initialView="home"/>);
  await user.click((await screen.findAllByRole('button',{name:'添加管理位置'}))[0]);
  expect(await screen.findByRole('alert')).toHaveTextContent('操作失败，请重试');
  await user.click(screen.getByRole('button',{name:'EN'}));
  expect(screen.getByRole('alert')).toHaveTextContent('Operation failed. Please try again.');
  await user.click(screen.getByRole('button',{name:'Dismiss'}));
  expect(screen.queryByRole('alert')).not.toBeInTheDocument();
});

it.each(['failure','no-title'] as const)('网页标题 %s 反馈切语言不重抓、不覆盖已编辑标题',async scenario=>{
  if(scenario==='failure')mocks.webMetadata.mockRejectedValue({message:'网站暂不可用，可手动填写标题',message_en:'The website is unavailable. Enter the title manually.'});
  else mocks.webMetadata.mockResolvedValue({title:null,final_url:'https://example.com'});
  const user=userEvent.setup();render(<App initialView="home"/>);
  await user.click(await screen.findByRole('button',{name:'添加网页'}));
  const dialog=screen.getByRole('dialog');
  await user.type(within(dialog).getByRole('textbox',{name:'网址'}),'https://example.com');
  await within(dialog).findByText(scenario==='failure'?'网站暂不可用，可手动填写标题':'网站未提供标题，可自行修改名称');
  const title=within(dialog).getByRole('textbox',{name:'标题，可修改'});
  await user.clear(title);await user.type(title,'我自己的标题');
  await user.click(screen.getByRole('button',{name:'EN'}));
  expect(within(dialog).getByText(scenario==='failure'?'The website is unavailable. Enter the title manually.':'No title provided. You can edit the name.')).toBeVisible();
  expect(title).toHaveValue('我自己的标题');
  expect(mocks.webMetadata).toHaveBeenCalledExactlyOnceWith('https://example.com');
  await user.click(within(dialog).getByRole('button',{name:'Save'}));
  expect(mocks.mutate).toHaveBeenCalledWith(expect.objectContaining({type:'save_web',name:'我自己的标题',url:'https://example.com'}));
});

it('拖入逐项错误保留双语，语言切换不重新导入',async()=>{
  mocks.importDrop.mockResolvedValue({items:[{path:'/fixture/failure.txt',status:'failed',resource_id:null,message:'对象已改变，请重新拖入',message_en:'The file changed. Drop it again.'}]});
  const user=userEvent.setup();render(<App initialView="home"/>);
  await within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).findByRole('button',{name:/^客户 A/});
  Object.defineProperty(document,'elementFromPoint',{configurable:true,value:()=>document.querySelector('main')});
  const start=mocks.listen.mock.calls.find(([name])=>name==='files-drop-started')![1];
  const ready=mocks.listen.mock.calls.find(([name])=>name==='files-dropped')![1];
  await act(async()=>{const event={payload:{token:'failed-drop',x:100,y:100,count:1}};start(event);ready(event);});
  await screen.findByText('failure.txt · 对象已改变，请重新拖入');
  await user.click(screen.getByRole('button',{name:'EN'}));
  expect(screen.getByText('failure.txt · The file changed. Drop it again.')).toBeVisible();
  expect(mocks.importDrop).toHaveBeenCalledOnce();
});

it('快速搜索失败随语言更新，切语言不重发搜索',async()=>{
  mocks.search.mockRejectedValue({message:'查询暂不可用',message_en:'Search is unavailable.'});
  const user=userEvent.setup();render(<App initialView="home"/>);
  await within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).findByRole('button',{name:/^客户 A/});
  await user.type(screen.getByRole('combobox'),'合同');
  expect(await screen.findByRole('alert')).toHaveTextContent('查询暂不可用');
  await user.click(screen.getByRole('button',{name:'EN'}));
  expect(screen.getByRole('alert')).toHaveTextContent('Search is unavailable.');
  expect(mocks.search).toHaveBeenCalledOnce();
});

it('批量提交前资料页变化，缺失名称使用当前语言占位并保留逐项失败',async()=>{
  mocks.overview.mockResolvedValue({...meta,workspaces:[...meta.workspaces,{id:'workspace2',name:'客户 B',count:0}]});
  mocks.resources.mockResolvedValue({...page,items:[file('a','资料A.txt'),file('b','资料B.txt')]});
  const user=userEvent.setup();render(<App initialView="home"/>);
  await user.click(await screen.findByRole('checkbox',{name:'选择 资料A.txt'}));
  await user.click(screen.getByRole('checkbox',{name:'选择 资料B.txt'}));
  await user.click(screen.getByRole('button',{name:'加入工作区'}));
  const dialog=screen.getByRole('dialog');await user.selectOptions(within(dialog).getByRole('combobox'),'workspace2');
  mocks.resources.mockResolvedValue(page);
  await act(async()=>mocks.listen.mock.calls.find(([name])=>name==='catalog-changed')![1]({payload:null}));
  await waitFor(()=>expect(screen.queryByRole('checkbox',{name:'选择 资料A.txt'})).not.toBeInTheDocument());
  mocks.mutate.mockRejectedValue({message:'原件未授权',message_en:'The original file is not authorized.'});
  await user.click(within(dialog).getByRole('button',{name:'保存'}));
  await screen.findByText('0 项成功 · 2 项失败');
  expect(screen.getAllByText('不可用资料',{selector:'.batch-report b'})).toHaveLength(2);
  await user.click(screen.getByRole('button',{name:'EN'}));
  expect(screen.getAllByText('Unavailable resource',{selector:'.batch-report b'})).toHaveLength(2);
  expect(screen.getAllByText('Unavailable resource',{selector:'.batch-report b'}).every(element=>element.parentElement?.textContent?.includes('The original file is not authorized.'))).toBe(true);
  expect(mocks.mutate).toHaveBeenCalledTimes(2);
});

it('多选五份资料后新建并加入只发出一个原子命令，成功进入新工作区',async()=>{
  const items=Array.from({length:5},(_,index)=>file('f'+index,`资料${index}.txt`));
  mocks.resources.mockImplementation(()=>Promise.resolve({...page,items}));
  mocks.mutate.mockImplementation(mutation=>{mocks.overview.mockResolvedValue({...meta,workspaces:[...meta.workspaces,{id:'created',name:mutation.name,count:5}]});return Promise.resolve({id:'created',changed:6});});
  const user=userEvent.setup();render(<App initialView="home"/>);
  for(const item of items)await user.click(await screen.findByRole('checkbox',{name:'选择 '+item.name}));
  await user.click(screen.getByRole('button',{name:'新建工作区并加入'}));
  const dialog=screen.getByRole('dialog');
  expect(within(dialog).getByText('将 5 项资料加入新工作区')).toBeVisible();
  await user.type(within(dialog).getByRole('textbox',{name:'名称'}),'新客户');
  await user.click(within(dialog).getByRole('button',{name:'保存'}));
  expect(mocks.mutate).toHaveBeenCalledExactlyOnceWith({type:'create_workspace',name:'新客户',resource_ids:items.map(item=>item.id)});
  expect(await screen.findByRole('heading',{level:1,name:'新客户'})).toBeVisible();
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  expect(screen.getAllByRole('checkbox').every(element=>!(element as HTMLInputElement).checked)).toBe(true);
  expect(mocks.resources).toHaveBeenLastCalledWith(expect.objectContaining({workspace_id:'created'}));
});

it('所选资料创建失败或取消保留原选择与名称，重试仍提交原UUID集合',async()=>{
  mocks.resources.mockResolvedValue({...page,items:[file('a','A.txt'),file('b','B.txt')]});
  mocks.mutate.mockRejectedValueOnce({message:'资源未授权',message_en:'Resource unauthorized.'}).mockResolvedValueOnce({id:'created',changed:3});
  const user=userEvent.setup();render(<App initialView="home"/>);
  await user.click(await screen.findByRole('checkbox',{name:'选择 A.txt'}));await user.click(screen.getByRole('checkbox',{name:'选择 B.txt'}));
  await user.click(screen.getByRole('button',{name:'新建工作区并加入'}));
  let dialog=screen.getByRole('dialog');await user.click(within(dialog).getByRole('button',{name:'取消'}));
  expect(mocks.mutate).not.toHaveBeenCalled();expect(screen.getByRole('checkbox',{name:'选择 A.txt'})).toBeChecked();
  await user.click(screen.getByRole('button',{name:'新建工作区并加入'}));dialog=screen.getByRole('dialog');
  await user.type(within(dialog).getByRole('textbox'),'保留名称');await user.click(within(dialog).getByRole('button',{name:'保存'}));
  expect(await within(dialog).findByRole('alert')).toHaveTextContent('资源未授权');
  expect(within(dialog).getByRole('textbox')).toHaveValue('保留名称');
  expect(screen.getByRole('checkbox',{name:'选择 A.txt'})).toBeChecked();
  expect(screen.getByRole('checkbox',{name:'选择 B.txt'})).toBeChecked();
  await user.click(within(dialog).getByRole('button',{name:'保存'}));
  await waitFor(()=>expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  expect(mocks.mutate.mock.calls.map(([mutation])=>mutation)).toEqual(Array(2).fill({type:'create_workspace',name:'保留名称',resource_ids:['a','b']}));
});

it('普通新建保持创建空工作区，不隐式消费列表选择',async()=>{
  mocks.resources.mockResolvedValue({...page,items:[file('a','A.txt')]});
  const user=userEvent.setup();render(<App initialView="home"/>);
  await user.click(await screen.findByRole('checkbox',{name:'选择 A.txt'}));
  await user.click(screen.getAllByRole('button',{name:'新建工作区'})[1]);
  const dialog=screen.getByRole('dialog');expect(within(dialog).queryByText(/项资料加入/)).not.toBeInTheDocument();
  await user.type(within(dialog).getByRole('textbox'),'空工作区');await user.click(within(dialog).getByRole('button',{name:'保存'}));
  expect(mocks.mutate).toHaveBeenCalledExactlyOnceWith({type:'create_workspace',name:'空工作区'});
});

it('分组展示仍默认高亮后端第一精确文件，回车不跳到前缀工作区',async()=>{
  const exact=file('exact','合同');
  mocks.search.mockImplementation(request=>Promise.resolve({request_id:request.request_id,page:{...page,items:[exact,{...file('workspace1','合同项目'),kind:'workspace'}, {...file('web','合同网页'),kind:'web'}]}}));
  const user=userEvent.setup();render(<App initialView="home"/>);
  await within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).findByRole('button',{name:/^客户 A/});
  const input=screen.getByRole('combobox',{name:'全局搜索'});await user.type(input,'合同');
  const option=await within(await screen.findByRole('group',{name:'文件'})).findByRole('option');
  expect(option).toHaveAttribute('aria-selected','true');
  expect(input).toHaveAttribute('aria-activedescendant',option.id);
  expect(screen.getAllByRole('option')[0]).toBe(option);
  await user.keyboard('{Enter}');expect(mocks.open).toHaveBeenCalledExactlyOnceWith('exact');
});

it('同名候选不默认猜测原件，明确方向键选择后才能回车打开',async()=>{
  mocks.search.mockImplementation(request=>Promise.resolve({request_id:request.request_id,page:{...page,items:[file('a','合同'),{...file('b','合同'),location:'/other/合同'}]}}));
  const user=userEvent.setup();render(<App initialView="home"/>);
  await within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).findByRole('button',{name:/^客户 A/});
  const input=screen.getByRole('combobox',{name:'全局搜索'});await user.type(input,'合同');await screen.findAllByRole('option');
  expect(input).not.toHaveAttribute('aria-activedescendant');
  expect(screen.getAllByRole('option').every(item=>item.getAttribute('aria-selected')==='false')).toBe(true);
  await user.keyboard('{Enter}');expect(mocks.open).not.toHaveBeenCalled();
  await user.keyboard('{ArrowDown}{Enter}');expect(mocks.open).toHaveBeenCalledExactlyOnceWith('a');
});

it('完整搜索可返回原工作区并恢复分类，查询不改变来源上下文',async()=>{
  mocks.search.mockImplementation(request=>Promise.resolve({request_id:request.request_id,page:{...page,items:[file('a','合同')]}}));
  mocks.searchPage.mockImplementation(request=>Promise.resolve({request_id:request.request.request_id,page}));
  const user=userEvent.setup();render(<App initialView="home"/>);
  await user.click(await within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).findByRole('button',{name:/^客户 A/}));
  await user.click(screen.getByRole('button',{name:'文件'}));
  await user.type(screen.getByRole('combobox',{name:'全局搜索'}),'合同');await screen.findByRole('option');
  await user.click(screen.getByRole('button',{name:'查看全部搜索结果'}));
  const back=await screen.findByRole('button',{name:'返回：客户 A'});
  await user.type(screen.getByRole('combobox',{name:'全局搜索'}),'新');await within(screen.getByRole('listbox')).findByRole('option');await user.keyboard('{Escape}');
  await user.click(back);
  expect(screen.getByRole('heading',{level:1})).toHaveTextContent('客户 A');
  await waitFor(()=>expect(mocks.resources).toHaveBeenLastCalledWith(expect.objectContaining({workspace_id:'workspace1',kind:'file'})));
  expect(screen.getByRole('combobox',{name:'全局搜索'})).toHaveValue('');
  expect(screen.queryByRole('button',{name:'返回：客户 A'})).not.toBeInTheDocument();
});

it('完整搜索返回收藏保留原模式，来源工作区已删除则返回列表',async()=>{
  mocks.search.mockImplementation(request=>Promise.resolve({request_id:request.request_id,page:{...page,items:[file('a','合同')]}}));
  mocks.searchPage.mockImplementation(request=>Promise.resolve({request_id:request.request.request_id,page}));
  const user=userEvent.setup();render(<App initialView="home"/>);
  await user.click(within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).getByRole('button',{name:'收藏'}));
  await user.type(screen.getByRole('combobox',{name:'全局搜索'}),'合同');await screen.findByRole('option');await user.click(screen.getByRole('button',{name:'查看全部搜索结果'}));
  await user.click(await screen.findByRole('button',{name:'返回收藏'}));
  await waitFor(()=>expect(mocks.resources).toHaveBeenLastCalledWith(expect.objectContaining({mode:'favorites'})));
  await user.click(within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).getByRole('button',{name:/^客户 A/}));
  await user.type(screen.getByRole('combobox',{name:'全局搜索'}),'合同');await screen.findByRole('option');await user.click(screen.getByRole('button',{name:'查看全部搜索结果'}));
  await screen.findByRole('button',{name:'返回：客户 A'});
  mocks.overview.mockResolvedValue({...meta,workspaces:[]});
  await act(async()=>mocks.listen.mock.calls.find(([name])=>name==='catalog-changed')![1]({payload:null}));
  await user.click(await screen.findByRole('button',{name:'返回工作区列表'}));
  expect(await screen.findByRole('heading',{level:1})).toHaveTextContent('每件工作，都有自己的位置。');
});

it.each(['query','kind','scope','sort'])('完整搜索更换%s清除旧选择，后续关联只包含当前所选',async change=>{
  const a=file('a','旧资料.txt');const b={...file('b','新网页'),kind:'web'};
  mocks.search.mockImplementation(request=>Promise.resolve({request_id:request.request_id,page:{...page,items:[a]}}));
  mocks.searchPage.mockImplementation(request=>Promise.resolve({request_id:request.request.request_id,page:{...page,items:[a]}}));
  const user=userEvent.setup();render(<App initialView="home"/>);
  await user.click(await within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).findByRole('button',{name:/^客户 A/}));
  await user.type(screen.getByRole('combobox',{name:'全局搜索'}),'资料');await screen.findByRole('option');await user.click(screen.getByRole('button',{name:'查看全部搜索结果'}));
  await user.click(await screen.findByRole('checkbox',{name:'选择 旧资料.txt'}));
  mocks.searchPage.mockImplementation(request=>Promise.resolve({request_id:request.request.request_id,page:{...page,items:[b]}}));
  if(change==='kind')await user.click(screen.getByRole('button',{name:'网页'}));
  else if(change==='sort')await user.selectOptions(screen.getByRole('combobox',{name:'搜索排序'}),'modified');
  else{
    if(change==='query')await user.type(screen.getByRole('textbox',{name:'完整搜索词'}),'新');else await user.selectOptions(screen.getByRole('combobox',{name:'工作区范围'}),'current');
  }
  await user.click(await screen.findByRole('checkbox',{name:'选择 新网页'}));
  expect(screen.getByText('1 项已选择')).toBeVisible();
  await user.click(screen.getByRole('button',{name:'加入工作区'}));
  const dialog=screen.getByRole('dialog');await user.selectOptions(within(dialog).getByRole('combobox'),'workspace1');await user.click(within(dialog).getByRole('button',{name:'保存'}));
  expect(mocks.mutate).toHaveBeenCalledExactlyOnceWith({type:'link',workspace_id:'workspace1',resource_ids:['b']});
});

it('搜索加载后页的选择在切换排序后清除，新的关联不夹带隐藏UUID',async()=>{
  const old=file('old','后页资料.txt');const fresh=file('fresh','新排序首页.txt');
  mocks.search.mockImplementation(request=>Promise.resolve({request_id:request.request_id,page:{...page,items:[old]}}));
  let sorted=false;
  mocks.searchPage.mockImplementation(request=>Promise.resolve({request_id:request.request.request_id,page:{...page,has_more:!sorted&&request.offset===0,items:sorted?[fresh]:request.offset===0?[file('first','原首页.txt')]:[old]},next_cursor:!sorted&&request.offset===0?cursor(100,'资料'):undefined}));
  const user=userEvent.setup();render(<App initialView="home"/>);
  await within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).findByRole('button',{name:/^客户 A/});
  await user.type(screen.getByRole('combobox',{name:'全局搜索'}),'资料');await screen.findByRole('option');await user.click(screen.getByRole('button',{name:'查看全部搜索结果'}));
  await user.click(await screen.findByRole('button',{name:'加载更多'}));
  await user.click(await screen.findByRole('checkbox',{name:'选择 后页资料.txt'}));
  sorted=true;await user.selectOptions(screen.getByRole('combobox',{name:'搜索排序'}),'name');
  await screen.findByRole('checkbox',{name:'选择 新排序首页.txt'});
  expect(screen.queryByText('1 项已选择')).not.toBeInTheDocument();
  await user.click(screen.getByRole('checkbox',{name:'选择 新排序首页.txt'}));
  await user.click(screen.getByRole('button',{name:'加入工作区'}));
  const dialog=screen.getByRole('dialog');await user.selectOptions(within(dialog).getByRole('combobox'),'workspace1');await user.click(within(dialog).getByRole('button',{name:'保存'}));
  expect(mocks.mutate).toHaveBeenCalledExactlyOnceWith({type:'link',workspace_id:'workspace1',resource_ids:['fresh']});
});

it('普通列表后续页版本变化重读首页时清除旧选择',async()=>{
  mocks.resources.mockResolvedValueOnce({...page,has_more:true,items:[file('old','旧页资料.txt')]})
    .mockResolvedValue({...page,revision:2,items:[file('fresh','新快照资料.txt')]});
  const user=userEvent.setup();render(<App initialView="home"/>);
  await user.click(await screen.findByRole('checkbox',{name:'选择 旧页资料.txt'}));
  await user.click(screen.getByRole('button',{name:'加载更多'}));
  await screen.findByRole('checkbox',{name:'选择 新快照资料.txt'});
  expect(screen.queryByText('1 项已选择')).not.toBeInTheDocument();
  expect(mocks.resources).toHaveBeenCalledTimes(3);
  expect(mocks.resources).toHaveBeenLastCalledWith(expect.objectContaining({offset:0}));
});

const managedRoot = (id:string,overrides:Partial<Overview['roots'][number]>={}):Overview['roots'][number] => ({id,path:`/fixture/${id}`,active:true,paused:false,scan_state:'completed',location_status:'available',scanned:1,failures:0,exclusions:[],...overrides});

it.each([
  {name:'空范围',roots:[],files:[],zh:'尚未添加管理位置',en:'No managed locations yet'},
  {name:'仅独立文件且失联仍保留授权',roots:[],files:[{...file('a','失联.txt'),status:'missing'},file('b','可用.txt')],zh:'已管理 2 个单独授权文件',en:'Managing 2 individually authorized files'},
  {name:'仅目录含暂停且忽略已撤权目录',roots:[managedRoot('a'),managedRoot('b',{paused:true,scan_state:'paused'}),managedRoot('revoked',{active:false,scan_state:'running'})],files:[],zh:'已管理 2 个目录',en:'Managing 2 folders'},
  {name:'混合目录与独立文件',roots:[managedRoot('a')],files:[file('a','合同.txt'),file('b','预算.txt')],zh:'已管理 1 个目录 · 2 个单独授权文件',en:'Managing 1 folder · 2 individually authorized files'},
  {name:'扫描中同时保留目录与独立文件数量',roots:[managedRoot('a',{scan_state:'running'})],files:[file('a','合同.txt')],zh:'正在索引 · 已管理 1 个目录 · 1 个单独授权文件',en:'Indexing · Managing 1 folder · 1 individually authorized file'},
  {name:'只有已撤权目录',roots:[managedRoot('revoked',{active:false})],files:[],zh:'尚未添加管理位置',en:'No managed locations yet'},
])('管理范围摘要：$name，中英文均使用真实授权数量',async({roots,files,zh,en})=>{
  mocks.overview.mockResolvedValue({...meta,roots,file_grants:files});
  const user=userEvent.setup();render(<App initialView="home"/>);
  expect(await screen.findByText(zh,{selector:'.catalog-status > span'})).toBeVisible();
  expect(screen.queryByText('已管理 0 个位置')).not.toBeInTheDocument();
  await user.click(screen.getByRole('button',{name:'EN'}));
  expect(await screen.findByText(en,{selector:'.catalog-status > span'})).toBeVisible();
  expect(mocks.pickRoot).not.toHaveBeenCalled();expect(mocks.mutate).not.toHaveBeenCalled();
});

it('单文件授权设置保留路径并用既有双语短文案区分失联与身份待确认',async()=>{
  mocks.overview.mockResolvedValue({...meta,file_grants:[
    {...file('missing','失联.txt'),status:'missing'},
    {...file('unconfirmed','待确认.txt'),status:'identity_unconfirmed'},
    file('available','正常.txt'),
  ]});
  const user=userEvent.setup();render(<App initialView="settings"/>);
  const card=(name:string)=>screen.getByText(name,{selector:'.location-copy > strong'}).closest('.location-card') as HTMLElement;
  await screen.findByText('失联.txt',{selector:'.location-copy > strong'});
  expect(within(card('失联.txt')).getByText('/fixture/失联.txt')).toBeVisible();
  expect(within(card('失联.txt')).getByText('原件暂不可用')).toBeVisible();
  expect(within(card('待确认.txt')).getByText('身份待确认')).toBeVisible();
  expect(within(card('正常.txt')).queryByText(/原件暂不可用|身份待确认/)).not.toBeInTheDocument();
  await user.click(screen.getByRole('button',{name:'EN'}));
  expect(within(card('失联.txt')).getByText('Original unavailable')).toBeVisible();
  expect(within(card('待确认.txt')).getByText('Identity unconfirmed')).toBeVisible();
  expect(within(card('正常.txt')).queryByText(/Original unavailable|Identity unconfirmed/)).not.toBeInTheDocument();
  expect(mocks.overview).toHaveBeenCalledOnce();expect(mocks.mutate).not.toHaveBeenCalled();
});

it.each([
  {name:'定位可用保留完成与失败统计',root:managedRoot('tracked',{scanned:7,failures:2}),zh:'已完成扫描 · 7 个文件 · 2 项不可访问',en:'Scan complete · 7 files · 2 unavailable'},
  {name:'定位可用保留暂停文案',root:managedRoot('paused',{paused:true,scan_state:'paused'}),zh:'已暂停扫描，已有授权保留',en:'Scanning paused; authorization retained'},
  {name:'旧目录即使暂停仍提示重新选择',root:managedRoot('legacy',{location_status:'legacy',paused:true,scan_state:'paused'}),zh:'请重新选择管理位置以启用目录定位',en:'Select this managed location again to enable folder tracking.'},
  {name:'定位失效优先于已完成扫描',root:managedRoot('unavailable',{location_status:'unavailable'}),zh:'目录暂不可用，请重新选择管理位置',en:'Folder unavailable. Select the managed location again.'},
  {name:'定位失效优先于暂停',root:managedRoot('unavailable-paused',{location_status:'unavailable',paused:true,scan_state:'paused'}),zh:'目录暂不可用，请重新选择管理位置',en:'Folder unavailable. Select the managed location again.'},
  {name:'已撤权优先于旧目录提示',root:managedRoot('revoked-legacy',{active:false,location_status:'legacy'}),zh:'已撤销授权，关联保留',en:'Authorization revoked; references retained'},
  {name:'已撤权优先于定位失效与暂停',root:managedRoot('revoked-unavailable',{active:false,location_status:'unavailable',paused:true,scan_state:'paused'}),zh:'已撤销授权，关联保留',en:'Authorization revoked; references retained'},
])('目录定位状态：$name，中英文不被其他状态掩盖',async({root,zh,en})=>{
  mocks.overview.mockResolvedValue({...meta,roots:[root]});
  const user=userEvent.setup();render(<App initialView="settings"/>);
  const location=await screen.findByText(root.path,{selector:'.location-copy > strong'});
  const card=location.closest('.location-card') as HTMLElement;
  expect(within(card).getByText(zh)).toBeVisible();
  await user.click(screen.getByRole('button',{name:'EN'}));
  expect(within(card).getByText(en)).toBeVisible();
  if(root.active&&root.location_status!=='available'){
    expect(within(card).queryByText(/Scan complete|Scanning paused/)).not.toBeInTheDocument();
    await user.click(screen.getByRole('button',{name:'Add location'}));
    expect(mocks.pickRoot).toHaveBeenCalledExactlyOnceWith('en');
  }
  if(!root.active)expect(within(card).queryByRole('button',{name:'Revoke'})).not.toBeInTheDocument();
  expect(mocks.mutate).not.toHaveBeenCalled();
});

it('首页unavailable目录保留授权数量且不把旧running状态写成正在索引',async()=>{
  mocks.overview.mockResolvedValue({...meta,roots:[managedRoot('retained',{location_status:'unavailable',scan_state:'running'})]});
  const user=userEvent.setup();render(<App initialView="home"/>);
  expect(await screen.findByText('已管理 1 个目录',{selector:'.catalog-status > span'})).toBeVisible();
  expect(screen.queryByText(/正在索引/,{selector:'.catalog-status > span'})).not.toBeInTheDocument();
  await user.click(screen.getByRole('button',{name:'EN'}));
  expect(screen.getByText('Managing 1 folder',{selector:'.catalog-status > span'})).toBeVisible();
  expect(mocks.pickRoot).not.toHaveBeenCalled();
});

it('legacy目录正在扫描时首页与搜索均显示索引提示，设置仍提示启用目录定位',async()=>{
  mocks.overview.mockResolvedValue({...meta,roots:[managedRoot('legacy',{location_status:'legacy',scan_state:'running'})]});
  const user=userEvent.setup();render(<App initialView="home"/>);
  expect(await screen.findByText('正在索引 · 已管理 1 个目录',{selector:'.catalog-status > span'})).toBeVisible();
  await user.click(screen.getByRole('combobox',{name:'全局搜索'}));
  expect(screen.getByText('索引中',{selector:'.search-footer span'})).toBeVisible();
  await user.click(screen.getByRole('button',{name:'EN'}));
  expect(screen.getByText('Indexing · Managing 1 folder',{selector:'.catalog-status > span'})).toBeVisible();
  await user.click(screen.getByRole('combobox',{name:'Global search'}));
  expect(screen.getByText('Indexing',{selector:'.search-footer span'})).toBeVisible();
  await user.click(screen.getByRole('button',{name:'Locations & settings'}));
  expect(await screen.findByText('Select this managed location again to enable folder tracking.')).toBeVisible();
  expect(screen.queryByText('Scanning · 1 files')).not.toBeInTheDocument();
  expect(mocks.pickRoot).not.toHaveBeenCalled();expect(mocks.mutate).not.toHaveBeenCalled();
});

const dashboardWorkspaces = [
  {id:'workspace1',name:'客户 A',count:2},
  {id:'workspace2',name:'客户 B',count:3},
  {id:'workspace3',name:'客户 C',count:1},
  {id:'workspace4',name:'客户 D',count:4},
];

it('初次默认个人工作台优先添加管理位置，取消不创建授权或工作区且保留创建入口',async()=>{
  mocks.overview.mockResolvedValue({...meta,workspaces:[]});
  const user=userEvent.setup();render(<App/>);
  await screen.findByRole('group',{name:'常用位置'});
  const heading=screen.getByRole('heading',{level:1,name:'个人工作台'});
  expect(heading).toBeVisible();
  const sidebar=screen.getByRole('complementary',{name:/主侧栏|Main sidebar/});const nav=within(sidebar).getByRole('navigation');
  expect(within(nav).getByRole('button',{name:'个人工作台'})).toHaveAttribute('aria-current','page');
  expect(screen.queryByRole('region',{name:'我的工作区'})).not.toBeInTheDocument();
  const pageTitle=heading.closest('.page-title') as HTMLElement;
  const addLocation=within(pageTitle).getByRole('button',{name:'添加管理位置'});
  expect(addLocation).toHaveClass('primary');
  expect(within(pageTitle).getByRole('button',{name:'新建工作区'})).not.toHaveClass('primary');
  await user.click(addLocation);
  expect(mocks.pickRoot).toHaveBeenCalledExactlyOnceWith('zh');
  await waitFor(()=>expect(within(pageTitle).getByRole('button',{name:'添加管理位置'})).toBeEnabled());
  expect(screen.getByText('尚未添加管理位置',{selector:'.catalog-status > span'})).toBeVisible();
  expect(screen.queryByRole('region',{name:'我的工作区'})).not.toBeInTheDocument();
  await user.click(within(pageTitle).getByRole('button',{name:'新建工作区'}));
  expect(within(screen.getByRole('dialog')).getByRole('textbox',{name:'名称'})).toBeVisible();
  await user.click(within(screen.getByRole('dialog')).getByRole('button',{name:'取消'}));
  await user.click(within(sidebar).getByRole('button',{name:'新建工作区'}));
  expect(within(screen.getByRole('dialog')).getByRole('textbox',{name:'名称'})).toBeVisible();
  await user.click(within(screen.getByRole('dialog')).getByRole('button',{name:'取消'}));
  await user.click(screen.getByRole('button',{name:'EN'}));
  await user.click(within(pageTitle).getByRole('button',{name:'Add location'}));
  expect(mocks.pickRoot).toHaveBeenNthCalledWith(2,'en');
  await waitFor(()=>expect(within(pageTitle).getByRole('button',{name:'Add location'})).toBeEnabled());
  expect(within(nav).queryByRole('button',{name:'All files'})).not.toBeInTheDocument();
  expect(screen.getByRole('heading',{level:1,name:'Personal workbench'})).toBeVisible();
  expect(within(nav).getByRole('button',{name:'Personal workbench'})).toHaveAttribute('aria-current','page');
  expect(screen.queryByRole('region',{name:'My workspaces'})).not.toBeInTheDocument();
  expect(mocks.mutate).not.toHaveBeenCalled();expect(mocks.importDrop).not.toHaveBeenCalled();
  expect(mocks.pickFiles).not.toHaveBeenCalled();expect(mocks.timeline).not.toHaveBeenCalled();
});

it('默认个人工作台为侧栏首菜单，展示前三工作区卡和内容顶部搜索，设置留在侧栏',async()=>{
  mocks.overview.mockResolvedValue({...meta,workspaces:dashboardWorkspaces});
  const user=userEvent.setup();render(<App/>);
  expect(screen.getByRole('heading',{level:1,name:'个人工作台'})).toBeVisible();
  const sidebar=screen.getByRole('complementary',{name:/主侧栏|Main sidebar/});const nav=within(sidebar).getByRole('navigation');
  const entries=[
    within(nav).getByRole('button',{name:'个人工作台'}),
    within(nav).getByRole('button',{name:'搜索与筛选'}),
    within(nav).getByRole('button',{name:/^工作区/}),
    within(nav).getByRole('button',{name:'收藏'}),
    within(nav).getByRole('button',{name:'动态'}),
  ];
  expect(within(nav).queryByRole('button',{name:'所有文件'})).not.toBeInTheDocument();
  expect(entries).toHaveLength(5);
  expect(entries[0]).toHaveAttribute('aria-current','page');
  for(let index=1;index<entries.length;index++){
    expect(entries[index-1].compareDocumentPosition(entries[index])&Node.DOCUMENT_POSITION_FOLLOWING).not.toBe(0);
    expect(entries[index]).not.toHaveAttribute('aria-current','page');
  }
  const workspaces=await screen.findByRole('region',{name:'我的工作区'});
  await waitFor(()=>expect(within(workspaces).getAllByRole('button').filter(button=>button.hasAttribute('data-workspace-id'))).toHaveLength(3));
  expect(within(workspaces).getAllByRole('button').filter(button=>button.hasAttribute('data-workspace-id')).map(button=>button.dataset.workspaceId)).toEqual(['workspace1','workspace2','workspace3']);
  expect(within(workspaces).queryByText('客户 D')).not.toBeInTheDocument();
  expect(within(workspaces).getByRole('button',{name:'查看全部工作区'})).toBeVisible();
  expect(screen.getByRole('combobox',{name:'全局搜索'}).closest('.topbar')).not.toBeNull();
  const settings=screen.getByRole('button',{name:'管理位置与设置'});
  expect(sidebar).toContainElement(settings);expect(screen.getAllByRole('button',{name:'管理位置与设置'})).toHaveLength(1);
  expect(mocks.timeline).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button',{name:'EN'}));
  expect(within(nav).queryByRole('button',{name:'All files'})).not.toBeInTheDocument();
  expect(screen.getByRole('heading',{level:1,name:'Personal workbench'})).toBeVisible();
  expect(within(nav).getByRole('button',{name:'Personal workbench'})).toHaveAttribute('aria-current','page');
  expect(screen.getByRole('region',{name:'My workspaces'})).toBeVisible();
  expect(sidebar).toContainElement(screen.getByRole('button',{name:'Locations & settings'}));
  expect(screen.getByRole('combobox',{name:'Global search'})).toBeVisible();
});

it('个人工作台最近打开筛选不改变全局菜单，并可从卡片或查看全部进入工作区',async()=>{
  mocks.overview.mockResolvedValue({...meta,workspaces:dashboardWorkspaces});
  mocks.resources.mockResolvedValue({...page,items:[file('recent','最近资料.txt')]});
  const user=userEvent.setup();render(<App/>);
  const nav=within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).getByRole('navigation');
  await user.click(within(screen.getByRole('main')).getByRole('button',{name:'最近打开'}));
  await waitFor(()=>expect(mocks.resources).toHaveBeenLastCalledWith(expect.objectContaining({workspace_id:null,mode:'recent'})));
  expect(screen.getByRole('heading',{level:1})).toHaveTextContent('个人工作台');
  expect(within(nav).getByRole('button',{name:'个人工作台'})).toHaveAttribute('aria-current','page');
  expect(within(nav).getByRole('button',{name:'收藏'})).not.toHaveAttribute('aria-current','page');
  const workspaces=screen.getByRole('region',{name:'我的工作区'});
  await user.click(within(workspaces).getByRole('button',{name:/客户 B/}));
  expect(screen.getByRole('heading',{level:1})).toHaveTextContent('客户 B');
  await waitFor(()=>expect(mocks.resources).toHaveBeenLastCalledWith(expect.objectContaining({workspace_id:'workspace2'})));
  await user.click(within(nav).getByRole('button',{name:'个人工作台'}));
  await user.click(await within(screen.getByRole('region',{name:'我的工作区'})).findByRole('button',{name:'查看全部工作区'}));
  expect(screen.getByRole('heading',{level:1})).toHaveTextContent('每件工作，都有自己的位置。');
  expect(within(nav).getByRole('button',{name:/^工作区/})).toHaveAttribute('aria-current','page');
});

it('动态只在明确导航时打开，重新挂载仍以个人工作台为默认入口',async()=>{
  const user=userEvent.setup();const first=render(<App/>);
  await within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).findByRole('button',{name:/^客户 A/});
  expect(screen.getByRole('heading',{level:1,name:'个人工作台'})).toBeVisible();
  expect(mocks.timeline).not.toHaveBeenCalled();
  const nav=within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).getByRole('navigation');
  await user.click(within(nav).getByRole('button',{name:'动态'}));
  expect(await screen.findByRole('region',{name:'文件活动时间线'})).toBeVisible();
  expect(within(nav).getByRole('button',{name:'动态'})).toHaveAttribute('aria-current','page');
  await waitFor(()=>expect(mocks.timeline).toHaveBeenCalled());
  first.unmount();mocks.timeline.mockClear();render(<App/>);
  await screen.findByRole('region',{name:'我的工作区'});
  expect(screen.getByRole('heading',{level:1,name:'个人工作台'})).toBeVisible();
  expect(within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).getByRole('button',{name:'个人工作台'})).toHaveAttribute('aria-current','page');
  expect(mocks.timeline).not.toHaveBeenCalled();
});

it.each([
  {view:'home' as const,mode:'recent',tab:'最近打开',back:'返回个人工作台',heading:'个人工作台',menu:'个人工作台'},
  {view:'home' as const,mode:'modified',tab:'最近修改',back:'返回个人工作台',heading:'个人工作台',menu:'个人工作台'},
  {view:'favorites' as const,mode:'favorites',tab:null,back:'返回收藏',heading:'收藏',menu:'收藏'},
])('完整搜索返回$view的$mode时恢复正确页面和资料模式',async({view,mode,tab,back,heading,menu})=>{
  mocks.search.mockImplementation(request=>Promise.resolve({request_id:request.request_id,page:{...page,items:[file('found','合同.txt')]}}));
  mocks.searchPage.mockImplementation(request=>Promise.resolve({request_id:request.request.request_id,page}));
  const user=userEvent.setup();render(<App initialView={view}/>);
  await within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).findByRole('button',{name:/^客户 A/});
  if(tab)await user.click(within(screen.getByRole('main')).getByRole('button',{name:tab}));
  await waitFor(()=>expect(mocks.resources).toHaveBeenLastCalledWith(expect.objectContaining({mode})));
  await user.type(screen.getByRole('combobox',{name:'全局搜索'}),'合同');await screen.findByRole('option');
  await user.click(screen.getByRole('button',{name:'查看全部搜索结果'}));
  await user.click(await screen.findByRole('button',{name:back}));
  expect(screen.getByRole('heading',{level:1})).toHaveTextContent(heading);
  expect(within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).getByRole('button',{name:menu})).toHaveAttribute('aria-current','page');
  await waitFor(()=>expect(mocks.resources).toHaveBeenLastCalledWith(expect.objectContaining({workspace_id:null,mode,offset:0})));
  if(view==='favorites'){
    const main=within(screen.getByRole('main'));
    for(const name of ['最近修改','最近使用','收藏','全部资料'])expect(main.queryByRole('button',{name})).not.toBeInTheDocument();
  }
  expect(screen.getByRole('combobox',{name:'全局搜索'})).toHaveValue('');
});

it('侧栏工作区保留UUID拖放入口，首页前三卡以外的工作区也可直接接收原生拖入',async()=>{
  mocks.overview.mockResolvedValue({...meta,workspaces:dashboardWorkspaces});mocks.importDrop.mockResolvedValue({items:[]});
  const user=userEvent.setup();render(<App/>);
  const sidebar=screen.getByRole('complementary',{name:/主侧栏|Main sidebar/});
  const target=await within(sidebar).findByRole('button',{name:/^客户 D/});
  expect(target).toHaveAttribute('data-workspace-id','workspace4');
  expect(within(screen.getByRole('region',{name:'我的工作区'})).queryByText('客户 D')).not.toBeInTheDocument();
  Object.defineProperty(document,'elementFromPoint',{configurable:true,value:()=>target});
  const start=mocks.listen.mock.calls.find(([name])=>name==='files-drop-started')![1];
  const ready=mocks.listen.mock.calls.find(([name])=>name==='files-dropped')![1];
  const payload={token:'sidebar-workspace',x:40,y:300,count:1};
  await act(async()=>{start({payload});ready({payload});});
  await waitFor(()=>expect(mocks.importDrop).toHaveBeenCalledExactlyOnceWith('sidebar-workspace','workspace4',false,'zh'));
  expect(screen.getByRole('heading',{level:1,name:'个人工作台'})).toBeVisible();
  expect(within(sidebar).getByRole('button',{name:'个人工作台'})).toHaveAttribute('aria-current','page');
  expect(mocks.mutate).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button',{name:'完成'}));
});

it.each([1,2])('原生物理坐标在%d倍屏按实际位置选择工作区入口和侧栏，换序后跟随UUID且优先于当前工作区',async(dpr)=>{
  const workspaces=[{id:'a',name:'坐标 A',count:0},{id:'b',name:'坐标 B',count:0},{id:'c',name:'坐标 C',count:0}];
  mocks.overview.mockResolvedValue({...meta,workspaces});mocks.importDrop.mockResolvedValue({items:[]});
  const user=userEvent.setup();render(<App/>);
  const sidebar=screen.getByRole('complementary',{name:/主侧栏|Main sidebar/});
  await user.click(await within(sidebar).findByRole('button',{name:/^坐标 C/}));
  const start=mocks.listen.mock.calls.find(([name])=>name==='files-drop-started')![1];
  const hover=mocks.listen.mock.calls.find(([name])=>name==='files-dragging')![1];
  const ready=mocks.listen.mock.calls.find(([name])=>name==='files-dropped')![1];
  const changed=mocks.listen.mock.calls.find(([name])=>name==='catalog-changed')![1];
  vi.stubGlobal('devicePixelRatio',dpr);
  try{
    // 坐标桩按几何区域命中真实DOM，而非无视坐标总返回同一按钮。
    Object.defineProperty(document,'elementFromPoint',{configurable:true,value:vi.fn((x:number,y:number)=>{
      if(x>=12&&x<183&&y>=454&&y<493)return sidebar.querySelectorAll('[data-workspace-id]')[1].querySelector('span');
      if(x>=611.5&&x<977&&y>=256&&y<402)return document.querySelectorAll('.workspace-drop-tray [data-workspace-id]')[1];
      return document.querySelector('main');
    })});
    for(const [index,point] of [[0,{x:800,y:330}],[1,{x:100,y:470}]] as const){
      const payload={token:`position-${index}`,x:point.x*dpr,y:point.y*dpr,count:1};
      await act(async()=>hover({payload}));
      expect(document.querySelector('.native-drop-target')).toHaveAttribute('data-workspace-id','b');
      await act(async()=>{start({payload});ready({payload});});
      await waitFor(()=>expect(mocks.importDrop).toHaveBeenLastCalledWith(payload.token,'b',false,'zh'));
      await within(sidebar).findByRole('button',{name:/^坐标 B/});
    }
    mocks.overview.mockResolvedValue({...meta,workspaces:[workspaces[1],workspaces[0],workspaces[2]]});
    await act(async()=>changed({payload:null}));
    await waitFor(()=>expect(sidebar.querySelectorAll('[data-workspace-id]')[1]).toHaveAttribute('data-workspace-id','a'));
    const payload={token:'reordered',x:100*dpr,y:470*dpr,count:1};
    await act(async()=>hover({payload}));expect(document.querySelector('.native-drop-target')).toHaveAttribute('data-workspace-id','a');
    await act(async()=>{start({payload});ready({payload});});
    await waitFor(()=>expect(mocks.importDrop).toHaveBeenLastCalledWith('reordered','a',false,'zh'));
    expect(document.querySelector('.native-drop-target')).toBeNull();
  }finally{vi.unstubAllGlobals();}
});

it.each(['home','search'] as const)('%s侧栏第13个以后工作区保留真实DOM目标，内部拖拽与原生拖入均按UUID关联',async(view)=>{
  const workspaces=Array.from({length:15},(_,index)=>({id:`workspace${index+1}`,name:`项目 ${index+1}`,count:0}));
  const resource=file('drag-resource','可拖拽资料.txt');
  mocks.overview.mockResolvedValue({...meta,workspaces});mocks.resources.mockResolvedValue({...page,items:[resource]});
  mocks.search.mockImplementation(request=>Promise.resolve({request_id:request.request_id,page:{...page,items:[resource]}}));
  mocks.searchPage.mockImplementation(request=>Promise.resolve({request_id:request.request.request_id,page:{...page,items:[resource]}}));
  mocks.importDrop.mockResolvedValue({items:[]});
  const user=userEvent.setup();render(<App/>);
  await screen.findByText(resource.name);
  if(view==='search'){
    await user.type(screen.getByRole('combobox',{name:'全局搜索'}),'资料');await screen.findByRole('option');
    await user.click(screen.getByRole('button',{name:'查看全部搜索结果'}));
    await screen.findByRole('heading',{level:1,name:'搜索结果：资料'});
  }else{
    const cards=screen.getByRole('region',{name:'我的工作区'});
    expect(cards.querySelectorAll('[data-workspace-id]')).toHaveLength(3);
    expect(within(cards).queryByText('项目 13')).not.toBeInTheDocument();
  }
  const sidebar=screen.getByRole('complementary',{name:/主侧栏|Main sidebar/});
  await within(sidebar).findByRole('button',{name:/^项目 15/});
  expect(sidebar.querySelectorAll('[data-workspace-id]')).toHaveLength(15);
  const target=within(sidebar).getByRole('button',{name:/^项目 13/});
  expect(target).toHaveAttribute('data-workspace-id','workspace13');
  const hit=target.querySelector('span')!;
  Object.defineProperty(document,'elementFromPoint',{configurable:true,value:()=>hit});
  vi.stubGlobal('PointerEvent',MouseEvent);
  try{
    const source=(await screen.findByText(resource.name)).closest('.resource-row')!;
    fireEvent.pointerDown(source,{button:0,clientX:400,clientY:300});
    fireEvent.pointerMove(window,{clientX:100,clientY:400});
    expect(target).toHaveClass('drop-target');expect(mocks.mutate).not.toHaveBeenCalled();
    fireEvent.pointerUp(window,{clientX:100,clientY:400});
    await waitFor(()=>expect(mocks.mutate).toHaveBeenCalledExactlyOnceWith({type:'link',workspace_id:'workspace13',resource_ids:['drag-resource']}));
  }finally{vi.unstubAllGlobals();}
  const nativeTarget=await within(sidebar).findByRole('button',{name:/^项目 15/});
  Object.defineProperty(document,'elementFromPoint',{configurable:true,value:()=>nativeTarget.querySelector('span')});
  const start=mocks.listen.mock.calls.find(([name])=>name==='files-drop-started')![1];
  const ready=mocks.listen.mock.calls.find(([name])=>name==='files-dropped')![1];
  const payload={token:`overflow-${view}`,x:40,y:300,count:1};
  await act(async()=>{start({payload});ready({payload});});
  await waitFor(()=>expect(mocks.importDrop).toHaveBeenCalledExactlyOnceWith(`overflow-${view}`,'workspace15',false,'zh'));
  expect(screen.getByRole('heading',{level:1})).toHaveTextContent(view==='home'?'个人工作台':'搜索结果：资料');
});

it.each(['empty','authorized'] as const)('默认首页读取%s范围期间保持中性状态，完成后才决定主要授权入口',async(scope)=>{
  let resolve!:(value:Overview)=>void;mocks.overview.mockImplementation(()=>new Promise(done=>{resolve=done;}));
  const user=userEvent.setup();render(<App/>);
  const pageTitle=screen.getByRole('heading',{level:1,name:'个人工作台'}).closest('.page-title') as HTMLElement;
  const status=document.querySelector('.catalog-status')!;
  expect(status).toHaveTextContent('正在读取资料');
  expect(status).not.toHaveTextContent('尚未添加管理位置');
  expect(within(pageTitle).queryByRole('button',{name:'添加管理位置'})).not.toBeInTheDocument();
  expect(screen.queryByRole('group',{name:'常用位置'})).not.toBeInTheDocument();
  expect(screen.queryByRole('region',{name:'我的工作区'})).not.toBeInTheDocument();
  await user.click(screen.getByRole('button',{name:'EN'}));
  expect(status).toHaveTextContent('Loading resources');
  expect(status).not.toHaveTextContent('No managed locations yet');
  expect(within(pageTitle).queryByRole('button',{name:'Add location'})).not.toBeInTheDocument();
  await act(async()=>resolve({...meta,workspaces:[],roots:scope==='authorized'?[managedRoot('existing')]:[]}));
  await waitFor(()=>expect(mocks.resources).toHaveBeenCalled());
  if(scope==='empty'){
    expect(status).toHaveTextContent('No managed locations yet');
    expect(within(pageTitle).getByRole('button',{name:'Add location'})).toHaveClass('primary');
    expect(await screen.findByRole('group',{name:'Common locations'})).toBeVisible();
  }else{
    expect(status).toHaveTextContent('Managing 1 folder');
    expect(within(pageTitle).queryByRole('button',{name:'Add location'})).not.toBeInTheDocument();
    expect(within(pageTitle).getByRole('button',{name:'New workspace'})).toHaveClass('primary');
    expect(screen.queryByRole('group',{name:'Common locations'})).not.toBeInTheDocument();
  }
  expect(mocks.pickRoot).not.toHaveBeenCalled();expect(mocks.mutate).not.toHaveBeenCalled();
});

it.each(['empty','authorized'] as const)('首次范围读取失败显示中英重试状态，重新读取成功后才呈现%s资料',async(scope)=>{
  let rejectOverview!:(reason:unknown)=>void;let resolveOverview!:(value:Overview)=>void;
  mocks.overview.mockImplementationOnce(()=>new Promise((_resolve,reject)=>{rejectOverview=reject;}))
    .mockImplementationOnce(()=>new Promise(resolve=>{resolveOverview=resolve;}));
  const resource=file('recovered','读取恢复的合同.txt');
  mocks.resources.mockResolvedValue({...page,items:scope==='authorized'?[resource]:[]});
  const user=userEvent.setup();render(<App/>);
  const status=document.querySelector('.catalog-status')!;
  expect(status).toHaveTextContent('正在读取资料');
  await act(async()=>rejectOverview({code:'database_error',message:'本地数据操作失败，请重试或检查备份',message_en:'The local data operation failed. Try again or check your backup.'}));
  expect(await screen.findByRole('alert')).toHaveTextContent('本地数据操作失败，请重试或检查备份');
  expect(status).toHaveTextContent('读取失败');
  expect(status).not.toHaveTextContent('正在读取资料');
  expect(status).not.toHaveTextContent('尚未添加管理位置');
  expect(status).toHaveAttribute('aria-busy','false');
  expect(screen.queryByRole('heading',{name:'文件原地不动，多一个更容易找的入口'})).not.toBeInTheDocument();
  expect(screen.queryByRole('group',{name:'常用位置'})).not.toBeInTheDocument();
  expect(screen.queryByRole('button',{name:'添加管理位置'})).not.toBeInTheDocument();
  expect(mocks.resources).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button',{name:'EN'}));
  expect(status).toHaveTextContent('Loading failed');
  expect(screen.getByRole('alert')).toHaveTextContent('The local data operation failed. Try again or check your backup.');
  expect(mocks.overview).toHaveBeenCalledOnce();
  await user.click(screen.getByRole('button',{name:'Try again'}));
  await waitFor(()=>expect(mocks.overview).toHaveBeenCalledTimes(2));
  expect(status).toHaveTextContent('Loading resources');
  expect(status).toHaveAttribute('aria-busy','true');
  expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  expect(screen.queryByRole('button',{name:'Add location'})).not.toBeInTheDocument();
  await act(async()=>resolveOverview({...meta,workspaces:[],roots:scope==='authorized'?[managedRoot('recovered')]:[]}));
  await waitFor(()=>expect(mocks.resources).toHaveBeenCalledOnce());
  expect(status).toHaveAttribute('aria-busy','false');
  if(scope==='authorized'){
    expect(await screen.findByText(resource.name)).toBeVisible();
    expect(status).toHaveTextContent('Managing 1 folder');
    expect(screen.queryByRole('button',{name:'Add location'})).not.toBeInTheDocument();
  }else{
    expect(await screen.findByRole('group',{name:'Common locations'})).toBeVisible();
    expect(status).toHaveTextContent('No managed locations yet');
  }
  expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  expect(mocks.rescan).not.toHaveBeenCalled();expect(mocks.mutate).not.toHaveBeenCalled();expect(mocks.pickRoot).not.toHaveBeenCalled();
});

it('已有资料的收藏操作失败不改成首次读取失败或触发重新加载',async()=>{
  const resource=file('loaded','读取正常的资料.txt');
  mocks.overview.mockResolvedValue({...meta,roots:[managedRoot('loaded')]});
  mocks.resources.mockResolvedValue({...page,items:[resource]});
  mocks.mutate.mockRejectedValue({message:'收藏未保存',message_en:'Favorite was not saved.'});
  const user=userEvent.setup();render(<App/>);
  await screen.findByText(resource.name);
  await user.click(screen.getByRole('button',{name:'收藏 '+resource.name}));
  expect(await screen.findByRole('alert')).toHaveTextContent('收藏未保存');
  const status=document.querySelector('.catalog-status')!;
  expect(status).toHaveTextContent('已管理 1 个目录');
  expect(status).not.toHaveTextContent('读取失败');
  expect(screen.getByText(resource.name)).toBeVisible();
  expect(screen.queryByRole('button',{name:'重新加载'})).not.toBeInTheDocument();
  expect(mocks.overview).toHaveBeenCalledOnce();expect(mocks.resources).toHaveBeenCalledOnce();
  expect(mocks.rescan).not.toHaveBeenCalled();
});


it('同页失效重读保留几何高度但立即清除旧资料，完成后释放高度',async()=>{
  mocks.resources.mockResolvedValue({...page,items:[file('private-old','授权变化前的合同.txt')]});
  render(<App initialView="home"/>);
  await screen.findByText('授权变化前的合同.txt');
  const main=screen.getByRole('main');
  vi.spyOn(main,'getBoundingClientRect').mockReturnValue({...main.getBoundingClientRect(),height:1800});
  let finish!:(value:Overview)=>void;
  mocks.overview.mockImplementationOnce(()=>new Promise<Overview>(resolve=>{finish=resolve;}));
  mocks.resources.mockResolvedValue({...page,permission_version:2,items:[]});
  vi.mocked(window.scrollTo).mockClear();
  const changed=mocks.listen.mock.calls.find(([name])=>name==='catalog-changed')![1];
  await act(async()=>changed({payload:null}));
  expect(screen.queryByText('授权变化前的合同.txt')).not.toBeInTheDocument();
  expect(main).toHaveStyle({minHeight:'1800px'});
  expect(screen.getByRole('heading',{name:'正在读取资料'})).toBeInTheDocument();
  await act(async()=>finish({...meta,permission_version:2}));
  await waitFor(()=>expect(main.style.minHeight).toBe(''));
  expect(screen.queryByText('授权变化前的合同.txt')).not.toBeInTheDocument();
  expect(window.scrollTo).not.toHaveBeenCalled();
});

it('仅扫描进度通知保留已加载搜索后页和选择，真实版本变化仍重读',async()=>{
  mocks.searchPage.mockImplementation(request=>Promise.resolve({request_id:request.request.request_id,page:{...page,has_more:request.offset===0,items:[file(request.offset?'tail':'head',request.offset?'后页合同':'首页合同')]},next_cursor:request.offset?undefined:cursor(100,'')}));
  const user=userEvent.setup();render(<App initialView="search"/>);fireEvent.change(screen.getByRole("textbox",{name:"完整搜索词"}),{target:{value:"合同"}});
  await screen.findByText('首页合同');await user.click(screen.getByRole('button',{name:'加载更多'}));
  await screen.findByText('后页合同');await user.click(screen.getByRole('checkbox',{name:'选择 后页合同'}));
  const progress=mocks.listen.mock.calls.find(([name])=>name==='scan-progress')![1];
  await act(async()=>progress({payload:null}));
  expect(mocks.searchPage).toHaveBeenCalledTimes(2);
  expect(screen.getByRole('checkbox',{name:'选择 后页合同'})).toBeChecked();
  mocks.overview.mockResolvedValue({...meta,revision:2});
  mocks.searchPage.mockImplementation(request=>Promise.resolve({request_id:request.request.request_id,page:{...page,revision:2,items:[file('fresh','更新合同')]}}));
  await act(async()=>progress({payload:null}));
  await screen.findByText('更新合同');expect(screen.queryByText('后页合同')).not.toBeInTheDocument();
});

it('迟到扫描进度不能覆盖撤权通知后的新概览或恢复旧名称',async()=>{
  mocks.resources.mockResolvedValue({...page,items:[file('old','旧授权合同')]});
  render(<App initialView="home"/>);await screen.findByText('旧授权合同');
  let finish!:(value:Overview)=>void;
  mocks.overview.mockImplementationOnce(()=>new Promise<Overview>(resolve=>{finish=resolve;}));
  const progress=mocks.listen.mock.calls.find(([name])=>name==='scan-progress')![1];
  await act(async()=>progress({payload:null}));
  mocks.overview.mockResolvedValue({...meta,permission_version:2,revision:2,workspaces:[{id:'new-space',name:'新授权工作区',count:0}]});
  mocks.resources.mockResolvedValue({...page,permission_version:2,revision:2,items:[]});
  const changed=mocks.listen.mock.calls.find(([name])=>name==='catalog-changed')![1];
  await act(async()=>changed({payload:null}));
  expect(screen.queryByText('旧授权合同')).not.toBeInTheDocument();
  await within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).findByRole('button',{name:/新授权工作区/});
  await act(async()=>finish(meta));
  expect(within(screen.getByRole('complementary',{name:/主侧栏|Main sidebar/})).getByRole('button',{name:/新授权工作区/})).toBeVisible();
  expect(screen.queryByText('旧授权合同')).not.toBeInTheDocument();
});

it('修正无效日期后成功查询清除旧查询错误，但保留操作失败',async()=>{
  mocks.searchPage.mockRejectedValueOnce({code:'invalid_input',message:'请选择有效的修改时间范围'})
    .mockImplementation(request=>Promise.resolve({request_id:request.request.request_id,page:{...page,items:[file('valid','有效日期资料')]}}));
  const user=userEvent.setup();render(<App initialView="search"/>);fireEvent.change(screen.getByRole("textbox",{name:"完整搜索词"}),{target:{value:"合同"}});
  expect(await screen.findByRole('alert')).toHaveTextContent('请选择有效的修改时间范围');
  fireEvent.change(screen.getByLabelText('修改日期，从'),{target:{value:'2026-11-21'}});
  await screen.findByRole('checkbox',{name:'选择 有效日期资料'});
  await waitFor(()=>expect(screen.queryByRole('alert')).not.toBeInTheDocument());
  mocks.mutate.mockRejectedValueOnce({message:'收藏操作失败'});
  await user.click(screen.getByRole('button',{name:'收藏 有效日期资料'}));
  expect(await screen.findByRole('alert')).toHaveTextContent('收藏操作失败');
  fireEvent.change(screen.getByLabelText('修改日期，从'),{target:{value:'2026-11-22'}});
  await waitFor(()=>expect(mocks.searchPage).toHaveBeenCalledTimes(3));
  expect(screen.getByRole('alert')).toHaveTextContent('收藏操作失败');
});
it('空词可进入完整搜索且不请求空词或自动列全库',async()=>{
  const user=userEvent.setup();render(<App/>);
  await user.click(screen.getByRole('button',{name:'搜索与筛选'}));
  expect(await screen.findByRole('heading',{name:'输入名称或路径'})).toBeVisible();
  expect(mocks.searchPage).not.toHaveBeenCalled();
  const count=mocks.resources.mock.calls.length;
  await user.click(screen.getByRole('combobox',{name:'全局搜索'}));
  await user.click(screen.getByRole('button',{name:'查看全部搜索结果'}));
  expect(mocks.searchPage).not.toHaveBeenCalled();expect(mocks.resources.mock.calls.length).toBe(count);
});
it('完整页合同与快捷招聘隔离，Esc保持筛选排序选择及已加载页',async()=>{
  mocks.search.mockImplementation(request=>Promise.resolve({request_id:request.request_id,page:{...page,items:[file('quick',request.query)]}}));
  mocks.searchPage.mockImplementation(request=>Promise.resolve({request_id:request.request.request_id,page:{...page,items:[file(String(request.offset),'合同'+request.offset)],has_more:request.offset===0},next_cursor:request.offset===0?cursor(100):undefined}));
  const user=userEvent.setup();render(<App/>);await user.click(screen.getByRole('button',{name:'搜索与筛选'}));
  await user.type(screen.getByRole('textbox',{name:'完整搜索词'}),'合同');await user.click(await screen.findByRole('checkbox',{name:'选择 合同0'}));
  await user.click(screen.getByRole('button',{name:'加载更多'}));await screen.findByText('合同100');
  const count=mocks.searchPage.mock.calls.length;
  await user.type(screen.getByRole('combobox',{name:'全局搜索'}),'招聘');await within(screen.getByRole('listbox')).findByRole('option');await user.keyboard('{Escape}');
  expect(screen.getByRole('textbox',{name:'完整搜索词'})).toHaveValue('合同');expect(screen.getByText('合同100')).toBeVisible();expect(screen.getByRole('checkbox',{name:'选择 合同0'})).toBeChecked();expect(mocks.searchPage).toHaveBeenCalledTimes(count);
  await user.click(screen.getByRole('combobox',{name:'全局搜索'}));await user.click(screen.getByRole('button',{name:'查看全部搜索结果'}));
  await waitFor(()=>expect(mocks.searchPage).toHaveBeenLastCalledWith(expect.objectContaining({offset:0,request:expect.objectContaining({query:'招聘'})})));
  expect(screen.queryByText('1 项已选择')).not.toBeInTheDocument();
});
it('组字不查中间词，候选Enter与改词后的Enter不打开旧命中',async()=>{
  mocks.search.mockImplementation(request=>Promise.resolve({request_id:request.request_id,page:{...page,items:[file('old','旧结果')]}}));
  const user=userEvent.setup();render(<App/>);const input=screen.getByRole('combobox',{name:'全局搜索'});await user.click(input);
  fireEvent.compositionStart(input);fireEvent.change(input,{target:{value:'hetong'}});
  await act(async()=>{await new Promise(resolve=>setTimeout(resolve,100));});expect(mocks.search).not.toHaveBeenCalled();
  fireEvent.change(input,{target:{value:'合同'}});fireEvent.compositionEnd(input);fireEvent.keyDown(input,{key:'Enter'});expect(mocks.open).not.toHaveBeenCalled();
  await screen.findByRole('option');fireEvent.change(input,{target:{value:'招聘'}});fireEvent.keyDown(input,{key:'Enter'});expect(mocks.open).not.toHaveBeenCalled();
  await screen.findByRole('option');expect(mocks.search.mock.calls.map(([r])=>r.query)).toEqual(['合同','招聘']);
});
it('普通打开事件后台更新不清列表或选择，双击仅一个IPC且权限变化仍强失效',async()=>{
  const item=file('kept','保持选择.txt');mocks.resources.mockResolvedValue({...page,items:[item]});
  let finish!:(value:ResourcePage)=>void;
  mocks.open.mockImplementation(async()=>{mocks.resources.mockImplementationOnce(()=>new Promise(resolve=>{finish=resolve;}));await mocks.listen.mock.calls.find(([name])=>name==='resource-opened')![1]({payload:null});});
  const user=userEvent.setup();render(<App/>);await user.click(await screen.findByRole('checkbox',{name:'选择 保持选择.txt'}));
  await user.dblClick(screen.getByRole('button',{name:/^保持选择.txt/}));await waitFor(()=>expect(finish).toBeTypeOf('function'));
  expect(mocks.open).toHaveBeenCalledExactlyOnceWith('kept');expect(screen.getByRole('checkbox',{name:'选择 保持选择.txt'})).toBeChecked();expect(screen.queryByRole('heading',{name:'正在读取资料'})).not.toBeInTheDocument();
  await act(async()=>finish({...page,items:[{...item,opened_at:1234}]}));expect(screen.getByRole('checkbox')).toBeChecked();
  mocks.resources.mockImplementation(()=>new Promise(()=>{}));await act(async()=>mocks.listen.mock.calls.find(([name])=>name==='catalog-changed')![1]({payload:null}));expect(screen.queryByText('保持选择.txt')).not.toBeInTheDocument();
});
it('连续普通打开的后台重读晚返回不能覆盖较新的有效列表',async()=>{
  mocks.resources.mockResolvedValue({...page,items:[file('a','打开前.txt')]});render(<App/>);await screen.findByText('打开前.txt');
  const pending:Array<(value:ResourcePage)=>void>=[];mocks.resources.mockImplementation(()=>new Promise(resolve=>pending.push(resolve)));const handler=mocks.listen.mock.calls.find(([name])=>name==='resource-opened')![1];
  await act(async()=>{void handler({payload:null});});await waitFor(()=>expect(pending).toHaveLength(1));await act(async()=>{void handler({payload:null});});await waitFor(()=>expect(pending).toHaveLength(2));
  await act(async()=>pending[1]({...page,items:[file('a','第二次打开已更新.txt')]}));await screen.findByText('第二次打开已更新.txt');await act(async()=>pending[0]({...page,items:[file('a','第一次打开迟到.txt')]}));expect(screen.getByText('第二次打开已更新.txt')).toBeVisible();expect(screen.queryByText('第一次打开迟到.txt')).not.toBeInTheDocument();
});
