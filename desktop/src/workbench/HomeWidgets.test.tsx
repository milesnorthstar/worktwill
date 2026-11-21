import {act,fireEvent,render,screen,waitFor} from '@testing-library/react';
import {beforeEach,expect,it,vi} from 'vitest';
import userEvent from '@testing-library/user-event';
import type {NoteWrite,ScratchNote} from '../contracts';
const mocks=vi.hoisted(()=>({notes:vi.fn(),writeNote:vi.fn(),listen:vi.fn(),onCloseRequested:vi.fn(),close:vi.fn()}));
vi.mock('../api',async original=>({...await original<typeof import('../api')>(),native:true,api:{notes:mocks.notes,writeNote:mocks.writeNote}}));
vi.mock('@tauri-apps/api/event',()=>({listen:mocks.listen}));
vi.mock('@tauri-apps/api/core',()=>({isTauri:()=>true}));
vi.mock('@tauri-apps/api/window',()=>({getCurrentWindow:()=>({onCloseRequested:mocks.onCloseRequested,close:mocks.close})}));
import HomeWidgets from './HomeWidgets';
beforeEach(()=>{vi.resetAllMocks();mocks.notes.mockResolvedValue({scope:'s:0',items:[]});mocks.listen.mockResolvedValue(()=>{});mocks.onCloseRequested.mockResolvedValue(()=>{});mocks.close.mockResolvedValue(undefined);});
it('保存中编辑必须等当前文本确认，保存串行且删除排队不复活',async()=>{
 const pending:Array<{request:NoteWrite;resolve:(note:ScratchNote)=>void}>=[];
 mocks.writeNote.mockImplementation(request=>new Promise(resolve=>pending.push({request,resolve})));
 const user=userEvent.setup();render(<HomeWidgets lang="zh" visible/>);await waitFor(()=>expect(screen.getByRole('button',{name:'新建便签'})).toBeEnabled());await user.click(screen.getByRole('button',{name:'新建便签'}));
 const text=screen.getByRole('textbox');fireEvent.change(text,{target:{value:'第一版'}});await waitFor(()=>expect(pending).toHaveLength(1));
 fireEvent.change(text,{target:{value:'第二版'}});await act(async()=>pending[0].resolve({id:pending[0].request.id,text:'第一版',pinned:false,version:1,updated_at:1}));
 await waitFor(()=>expect(pending).toHaveLength(2));expect(screen.queryByText('已保存')).not.toBeInTheDocument();expect(pending[1].request.expected_version).toBe(1);expect(pending[1].request.text).toBe('第二版');
 await user.click(screen.getByRole('button',{name:'删除便签'}));await act(async()=>pending[1].resolve({id:pending[0].request.id,text:'第二版',pinned:false,version:2,updated_at:2}));await waitFor(()=>expect(pending).toHaveLength(3));expect(pending[2].request.deleted).toBe(true);
 await act(async()=>pending[2].resolve({id:pending[0].request.id,text:'第二版',pinned:false,version:3,updated_at:3}));expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
});
it('失败保留草稿及相同UUID版本，明确重试后才显示已保存',async()=>{
 mocks.writeNote.mockRejectedValueOnce({message:'存储失败'}).mockImplementation(request=>Promise.resolve({id:request.id,text:request.text,pinned:request.pinned,version:request.expected_version+1,updated_at:1}));
 const user=userEvent.setup();render(<HomeWidgets lang="zh" visible/>);await waitFor(()=>expect(screen.getByRole('button',{name:'新建便签'})).toBeEnabled());await user.click(screen.getByRole('button',{name:'新建便签'}));fireEvent.change(screen.getByRole('textbox'),{target:{value:'失败仍保留'}});
 await screen.findByRole('alert');expect(screen.getByRole('textbox')).toHaveValue('失败仍保留');expect(screen.queryByText('已保存')).not.toBeInTheDocument();await user.click(screen.getByRole('button',{name:'重试'}));await screen.findByText('已保存');expect(mocks.writeNote.mock.calls[1][0]).toEqual(mocks.writeNote.mock.calls[0][0]);
});
it('迟到读取不能覆盖已确认新版本或复活已删除便签',async()=>{
 const note:ScratchNote={id:'existing',text:'旧版',pinned:false,version:1,updated_at:1};mocks.notes.mockResolvedValue({scope:'s:0',items:[note]});mocks.writeNote.mockImplementation(request=>Promise.resolve({...note,text:request.text,version:request.expected_version+1,updated_at:2}));
 const user=userEvent.setup();render(<HomeWidgets lang="zh" visible/>);await screen.findByDisplayValue('旧版');let finish!:(value:unknown)=>void;mocks.notes.mockImplementationOnce(()=>new Promise(resolve=>{finish=resolve;}));await act(async()=>mocks.listen.mock.calls.find(([name])=>name==='notes-changed')![1]({}));fireEvent.change(screen.getByRole('textbox'),{target:{value:'已确认新版'}});await screen.findByText('已保存');await waitFor(()=>expect(mocks.writeNote).toHaveBeenCalled());
 await act(async()=>finish({scope:'s:0',items:[note]}));expect(screen.getByRole('textbox')).toHaveValue('已确认新版');
 mocks.notes.mockImplementationOnce(()=>new Promise(resolve=>{finish=resolve;}));await act(async()=>mocks.listen.mock.calls.find(([name])=>name==='notes-changed')![1]({}));await user.click(screen.getByRole('button',{name:'删除便签'}));await waitFor(()=>expect(screen.queryByRole('textbox')).not.toBeInTheDocument());await act(async()=>finish({scope:'s:0',items:[{...note,text:'已确认新版',version:2}]}));expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
});

it('退出等待已在途保存和最新草稿，只在当前版本确认后关闭',async()=>{
 const note:ScratchNote={id:'existing',text:'旧版',pinned:false,version:1,updated_at:1};mocks.notes.mockResolvedValue({scope:'s:0',items:[note]});
 const pending:Array<{request:NoteWrite;resolve:(note:ScratchNote)=>void}>=[];mocks.writeNote.mockImplementation(request=>new Promise(resolve=>pending.push({request,resolve})));
 render(<HomeWidgets lang="zh" visible/>);await screen.findByDisplayValue('旧版');fireEvent.change(screen.getByRole('textbox'),{target:{value:'自动保存中'}});await waitFor(()=>expect(pending).toHaveLength(1));
 fireEvent.change(screen.getByRole('textbox'),{target:{value:'退出前最新草稿'}});const event={preventDefault:vi.fn()};let closing!:Promise<void>;await act(async()=>{closing=mocks.onCloseRequested.mock.calls[0][0](event);});expect(event.preventDefault).toHaveBeenCalledOnce();expect(pending).toHaveLength(1);expect(mocks.close).not.toHaveBeenCalled();
 await act(async()=>pending[0].resolve({...note,text:'自动保存中',version:2}));await waitFor(()=>expect(pending).toHaveLength(2));expect(pending[1].request.text).toBe('退出前最新草稿');expect(mocks.close).not.toHaveBeenCalled();
 await act(async()=>{pending[1].resolve({...note,text:'退出前最新草稿',version:3});await closing;});expect(mocks.close).toHaveBeenCalledOnce();expect(screen.getByRole('textbox')).toHaveValue('退出前最新草稿');expect(screen.getByText('已保存')).toBeVisible();
});
