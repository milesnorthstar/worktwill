import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { EmptyState, ResourceList, SearchPanel } from './components';
import type { Resource } from '../contracts';

const t = (zh: string) => zh;
const file: Resource = { id: 'file1', kind: 'file', name: '合作合同_张三.docx', location: '/fixture/合同.docx', status: 'available', modified_at: 1, size: 10, favorite: false, workspace_ids: [] };

describe('资料列表的打开与权限行为', () => {
  const props = () => ({ t, lang: 'zh' as const, items: [file], selected: new Set<string>(), busy: false, onToggle: vi.fn(), onOpen: vi.fn(), onFavorite: vi.fn(), onReveal: vi.fn(), onRelink: vi.fn(), onEdit: vi.fn() });
  it('单击选择、双击只打开一次；不会连续调用系统打开', async () => {
    const p = props(); const user = userEvent.setup(); render(<ResourceList {...p} />);
    const name = screen.getByRole('button', { name: /^合作合同_张三/ });
    await user.click(name); expect(p.onOpen).not.toHaveBeenCalled();
    await user.dblClick(name); expect(p.onOpen).toHaveBeenCalledExactlyOnceWith(file);
    await user.click(screen.getByRole('button', { name: `打开 ${file.name}` }));
    expect(p.onOpen).toHaveBeenCalledTimes(2);
  });
  it('撤权资料可移除引用，但不提供打开或显示原始位置入口', async () => {
    const p = props(); p.items = [{ ...file, status: 'permission_denied', location: '', name: '未授权的文件' }]; render(<ResourceList {...p} />);
    await userEvent.dblClick(screen.getByRole('button', { name: /^未授权的文件/ }));
    expect(p.onOpen).not.toHaveBeenCalled();
    expect(screen.queryByRole('button', { name: /^打开 / })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: '重新指向' })).toBeVisible();
    expect(screen.getByRole('checkbox')).toBeEnabled();
  });
});

describe('搜索与空状态', () => {
  it('搜索分组区分工作区、文件与网页，提供可访问的高亮项', () => {
    render(<SearchPanel t={t} query="合同" searching={false} results={[{ ...file, id: 'w', kind: 'workspace', name: '合同' }, file, { ...file, id: 'url', kind: 'web', name: '合同后台' }]} active={1} scope="all" workspace="w" addIntent={false} hasLocations scanning={false} onScope={vi.fn()} onActive={vi.fn()} onActivate={vi.fn()} />);
    expect(within(screen.getByRole('group', { name: '文件' })).getByRole('option')).toHaveAttribute('aria-selected', 'true');
    expect(screen.getAllByRole('group')).toHaveLength(3);
    expect(screen.getByRole('listbox')).toHaveAccessibleName('搜索结果');
  });
  it('空工作区提供加入资料入口，避免误导用户重复授权', () => {
    render(<EmptyState t={t} view="workspace" mode="modified" kind="all" hasLocations loading={false} disabled={false} onLocation={vi.fn()} onAdd={vi.fn()} onWeb={vi.fn()} />);
    expect(screen.getByRole('heading', { name: '这个工作区还没有资料' })).toBeVisible();
    expect(screen.getByRole('button', { name: '添加文件' })).toBeEnabled();
    expect(screen.queryByRole('button', { name: '添加管理位置' })).not.toBeInTheDocument();
  });
  it('收藏没有结果时提示星标操作，加载状态不提供重复操作入口', () => {
    const props = { t, view: 'home', mode: 'favorites', kind: 'all', hasLocations: true, loading: false, disabled: false, onLocation: vi.fn(), onAdd: vi.fn(), onWeb: vi.fn() };
    const { rerender } = render(<EmptyState {...props} />);
    expect(screen.getByRole('heading', { name: '还没有收藏的资料' })).toBeVisible();
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
    rerender(<EmptyState {...props} loading />);
    expect(screen.getByRole('heading', { name: '正在读取资料' })).toBeVisible();
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
  });

  it.each([['search','all'],['workspace','file'],['workspace','web']])('读取中的%s/%s不提前显示无匹配或分类空态', (view,kind) => {
    const props={t,view,kind,mode:'modified',hasLocations:true,loading:true,disabled:false,onLocation:vi.fn(),onAdd:vi.fn(),onWeb:vi.fn()};
    const {rerender}=render(<EmptyState {...props}/>);
    expect(screen.getByRole('status')).toHaveAttribute('aria-busy','true');
    expect(screen.getByRole('heading',{name:'正在读取资料'})).toBeVisible();
    expect(screen.queryByText('没有匹配的资料')).not.toBeInTheDocument();
    expect(screen.queryByText('此分类下暂无资料')).not.toBeInTheDocument();
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
    rerender(<EmptyState {...props} loading={false}/>);
    expect(screen.getByRole('heading',{name:view==='search'?'没有匹配的资料':'此分类下暂无资料'})).toBeVisible();
  });
});

it('大型资料列表只渲染可见行，滚动后保持总高度并更新资料',()=>{
  const items=Array.from({length:250},(_,index)=>({...file,id:String(index),name:`资料${index}.txt`}));
  render(<ResourceList t={t} lang="zh" items={items} selected={new Set()} busy={false} onToggle={vi.fn()} onOpen={vi.fn()} onFavorite={vi.fn()} onReveal={vi.fn()} onRelink={vi.fn()} onEdit={vi.fn()}/>);
  expect(screen.getAllByRole('checkbox')).toHaveLength(16);
  const viewport=screen.getByRole('region',{name:'资料列表'});
  fireEvent.scroll(viewport,{target:{scrollTop:18000}});
  expect(screen.queryByRole('button',{name:/^资料0.txt/})).not.toBeInTheDocument();
  expect(screen.getAllByRole('checkbox').length).toBeLessThanOrEqual(16);
  expect(screen.getByRole('button',{name:/^资料210.txt/})).toBeInTheDocument();
});

it('首个完整页即可在同一滚动区操作，追加后三项保持末尾位置',()=>{
  const items=Array.from({length:103},(_,index)=>({...file,id:String(index),name:`分页${index}.txt`}));
  const props={t,lang:'zh' as const,selected:new Set<string>(),busy:false,onToggle:vi.fn(),onOpen:vi.fn(),onFavorite:vi.fn(),onReveal:vi.fn(),onRelink:vi.fn(),onEdit:vi.fn()};
  const {rerender}=render(<ResourceList {...props} items={items.slice(0,100)}/>);
  const viewport=screen.getByRole('region',{name:'资料列表'});
  expect(viewport).toHaveAttribute('tabindex','0');
  fireEvent.scroll(viewport,{target:{scrollTop:7896}});
  expect(screen.getByRole('checkbox',{name:'选择 分页99.txt'})).toBeInTheDocument();
  rerender(<ResourceList {...props} items={items}/>);
  expect(screen.getByRole('region',{name:'资料列表'})).toBe(viewport);
  expect(viewport.scrollTop).toBe(7896);
  expect(screen.getByRole('checkbox',{name:'选择 分页99.txt'})).toBeInTheDocument();
  expect(screen.getByRole('checkbox',{name:'选择 分页102.txt'})).toBeInTheDocument();
  expect(screen.queryByRole('checkbox',{name:'选择 分页0.txt'})).not.toBeInTheDocument();
});

it('工作区搜索结果直接导航，不提供资源勾选或拖入关联',async()=>{
  const open=vi.fn();const drag=vi.fn();
  render(<ResourceList t={t} lang="zh" items={[{...file,kind:'workspace',name:'项目工作区'}]} selected={new Set()} busy={false} onToggle={vi.fn()} onOpen={open} onFavorite={vi.fn()} onReveal={vi.fn()} onRelink={vi.fn()} onEdit={vi.fn()} onDragStart={drag}/>);
  expect(screen.getByRole('checkbox')).toBeDisabled();
  await userEvent.click(screen.getByRole('button',{name:/^项目工作区/}));expect(open).toHaveBeenCalledOnce();expect(drag).not.toHaveBeenCalled();
});

it('混合结果显示各类数量并可直接跳到网页，避免被长文件组挡住',async()=>{
  const scroll=vi.fn();const original=HTMLElement.prototype.scrollIntoView;HTMLElement.prototype.scrollIntoView=scroll;
  render(<SearchPanel t={t} query="合同" searching={false} results={[{...file,id:'w',kind:'workspace'},...Array.from({length:20},(_,i)=>({...file,id:'f'+i})),{...file,id:'u',kind:'web'}]} active={-1} scope="all" workspace={null} addIntent={false} hasLocations scanning={false} onScope={vi.fn()} onActive={vi.fn()} onActivate={vi.fn()}/>);
  expect(screen.getByRole('button',{name:'跳到文件结果'})).toHaveTextContent('文件20');
  await userEvent.click(screen.getByRole('button',{name:'跳到网页结果'}));expect(scroll).toHaveBeenCalledWith({block:'start'});
  expect(scroll.mock.instances[0]).toBe(screen.getByRole('group',{name:'网页'}));HTMLElement.prototype.scrollIntoView=original;
});

it('撤权占位随语言更新，可用资料的同名中文文件保持原文',()=>{
  const denied={...file,status:'permission_denied',name:'未授权的文件',location:''};
  const p={items:[denied],selected:new Set<string>(),busy:false,onToggle:vi.fn(),onOpen:vi.fn(),onFavorite:vi.fn(),onReveal:vi.fn(),onRelink:vi.fn(),onEdit:vi.fn()};
  const {rerender}=render(<ResourceList {...p} lang="en" t={(_zh,en)=>en}/>);
  expect(screen.getByRole('checkbox',{name:'Select Unauthorized file'})).toBeEnabled();
  expect(screen.getByRole('button',{name:/^Unauthorized file/})).toBeVisible();
  expect(screen.queryByText('未授权的文件')).not.toBeInTheDocument();
  rerender(<ResourceList {...p} lang="zh" t={t}/>);
  expect(screen.getByRole('checkbox',{name:'选择 未授权的文件'})).toBeInTheDocument();
  rerender(<ResourceList {...p} items={[{...file,name:'未授权的文件'}]} lang="en" t={(_zh,en)=>en}/>);
  expect(screen.getByRole('button',{name:/^未授权的文件/})).toBeVisible();
  expect(screen.getByRole('checkbox',{name:'Select 未授权的文件'})).toBeInTheDocument();
});
