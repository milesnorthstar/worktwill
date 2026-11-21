import {render,screen} from '@testing-library/react';
import {describe,expect,it,vi,afterEach} from 'vitest';
import {MatchText,resourceSize,resourceTime} from './ResourcePresentation';
import {ResourceList} from './components';
import type {Resource} from '../contracts';

const file:Resource={id:'f',kind:'file',name:'合作合同.docx',location:'/资料/合同/合作合同.docx',status:'available',size:131072,modified_at:1,favorite:false,workspace_ids:['w'],match_field:'path'};
afterEach(()=>vi.useRealTimers());

describe('资料命中展示',()=>{
  it('只高亮后端指定字段，保持名称和路径完整且不把查询当正则',()=>{
    const {container,rerender}=render(<MatchText text="合同[1].合同" query="[1]." enabled/>);
    expect(container.textContent).toBe('合同[1].合同');expect(container.querySelector('mark')).toHaveTextContent('[1].');
    rerender(<MatchText text="合同" query="合同"/>);expect(container.querySelector('mark')).toBeNull();
    rerender(<ResourceList t={zh=>zh} lang="zh" query="合同" items={[file]} workspaces={[{id:'w',name:'项目A'}]} selected={new Set()} busy={false} onToggle={vi.fn()} onOpen={vi.fn()} onFavorite={vi.fn()} onReveal={vi.fn()} onRelink={vi.fn()} onEdit={vi.fn()}/>);
    expect(container.querySelector('.resource-name strong mark')).toBeNull();expect(container.querySelector('.resource-name small mark')).toHaveTextContent('合同');
    expect(container.querySelector('.resource-workspaces')).toHaveTextContent('项目A');expect(screen.getByText('128 KB')).toBeVisible();
  });
  it('NFC和大小写展开后高亮不截断或吞掉原名称',()=>{
    const {container,rerender}=render(<MatchText text={'Cafe\u0301合同.docx'} query="CAFÉ" enabled/>);
    expect(container.querySelector('mark')).toHaveTextContent('Café');expect(container.textContent).toBe('Café合同.docx');
    rerender(<MatchText text="İ合同.docx" query="合同" enabled/>);expect(container.querySelector('mark')).toHaveTextContent('合同');expect(container.textContent).toBe('İ合同.docx');
    rerender(<MatchText text="İ合同.docx" query="i" enabled/>);expect(container.querySelector('mark')).toHaveTextContent('İ');expect(container.textContent).toBe('İ合同.docx');
  });
  it('撤权占位不显示新加入的大小、时间、工作区或真实文件类型',()=>{
    const {container}=render(<ResourceList t={zh=>zh} lang="zh" query="合同" items={[{...file,status:'permission_denied',name:'未授权的文件',location:''}]} workspaces={[{id:'w',name:'保密项目'}]} selected={new Set()} busy={false} onToggle={vi.fn()} onOpen={vi.fn()} onFavorite={vi.fn()} onReveal={vi.fn()} onRelink={vi.fn()} onEdit={vi.fn()}/>);
    expect(container.querySelector('.resource-workspaces')).toBeEmptyDOMElement();expect(container.querySelector('.resource-row .resource-size')).toBeEmptyDOMElement();
    expect(container).not.toHaveTextContent('保密项目');expect(container.querySelector('mark')).toBeNull();expect(container.querySelector('.resource-icon.word')).toBeNull();
    expect(screen.getByRole('button',{name:/未授权的文件/})).toBeDisabled();
  });
});

it('大小只描述文件；最近打开和修改时间仍由调用者传入各自事实',()=>{
  vi.useFakeTimers();vi.setSystemTime(new Date(2026,10,21,15,0));
  expect(resourceSize(file)).toBe('128 KB');expect(resourceSize({...file,size:0})).toBe('0 B');expect(resourceSize({...file,kind:'web'})).toBe('');
  expect(resourceTime(new Date(2026,10,21,9,42).getTime(),'zh')).toBe('今天 09:42');
  expect(resourceTime(new Date(2026,10,20,18,25).getTime(),'en')).toBe('Yesterday 18:25');
  expect(resourceTime(undefined,'zh')).toBe('—');
});
