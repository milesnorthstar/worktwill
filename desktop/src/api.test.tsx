import {beforeEach,expect,it,vi} from 'vitest';
const mock=vi.hoisted(()=>({invoke:vi.fn()}));
vi.mock('@tauri-apps/api/core',()=>({invoke:mock.invoke,isTauri:()=>true}));
beforeEach(()=>{vi.resetModules();mock.invoke.mockReset();});
const quick={request_id:1,query:'合同',kind:'all',workspace_id:null};

it('并行快速/完整搜索及取消共用一次原生会话初始化',async()=>{
  let resolve!:(value:string)=>void;
  mock.invoke.mockImplementation((command,args)=>command==='begin_search_session'?new Promise(done=>{resolve=done;}):Promise.resolve(command==='cancel_search'?true:{request_id:args.request.request_id??args.request.request.request_id,page:{items:[]}}));
  const {api}=await import('./api');
  const a=api.search(quick);const b=api.searchPage({request:{...quick,request_id:2},offset:0,sort:'relevance'});const c=api.cancelSearch('quick',1);
  expect(mock.invoke).toHaveBeenCalledExactlyOnceWith('begin_search_session');
  resolve('native-session');await Promise.all([a,b,c]);
  expect(mock.invoke).toHaveBeenCalledWith('search',{request:quick,session:'native-session'});
  expect(mock.invoke).toHaveBeenCalledWith('search_page',expect.objectContaining({session:'native-session'}));
  expect(mock.invoke).toHaveBeenCalledWith('cancel_search',{lane:'quick',requestId:1,session:'native-session'});
});
it('组件重新挂载的模块缓存保持ID递增，WebView新模块初始化新原生会话',async()=>{
  let sessions=0;mock.invoke.mockImplementation((command)=>Promise.resolve(command==='begin_search_session'?`session-${++sessions}`:{}));
  const first=await import('./api');expect(first.nextSearchRequestId()).toBe(1);await first.api.search(quick);
  const same=await import('./api');expect(same.nextSearchRequestId()).toBe(2);await same.api.search({...quick,request_id:2});
  expect(sessions).toBe(1);
  vi.resetModules();const fresh=await import('./api');expect(fresh.nextSearchRequestId()).toBe(1);await fresh.api.search(quick);
  expect(sessions).toBe(2);
  expect(mock.invoke).toHaveBeenLastCalledWith('search',{request:quick,session:'session-2'});
});
it('初始化错误明确返回，下一次明确查询才重试',async()=>{
  let begins=0;mock.invoke.mockImplementation((command)=>{
    if(command==='begin_search_session')return ++begins===1?Promise.reject({code:'internal_error',message:'初始化失败'}):Promise.resolve('retry-session');
    return Promise.resolve({request_id:2,page:{items:[]}});
  });
  const {api}=await import('./api');await expect(api.search(quick)).rejects.toEqual(expect.objectContaining({code:'internal_error'}));
  expect(begins).toBe(1);expect(await api.cancelSearch('quick',1)).toBe(false);expect(begins).toBe(1);await api.search({...quick,request_id:2});expect(begins).toBe(2);
});

it('没有搜索会话时取消不会创建会话或发出无效请求',async()=>{
  const {api}=await import('./api');expect(await api.cancelSearch('page',1)).toBe(false);expect(mock.invoke).not.toHaveBeenCalled();
});

it('原生选择和确认每次携带当前语言，既有目标和授权意图原样传递',async()=>{
  mock.invoke.mockResolvedValue(null);
  const {api}=await import('./api');
  for(const language of ['zh','en'] as const){
    await api.pickRoot(language);expect(mock.invoke).toHaveBeenLastCalledWith('pick_root',{language,suggestion:null});
    await api.pickFiles('workspace',language);expect(mock.invoke).toHaveBeenLastCalledWith('pick_files',{workspaceId:'workspace',language});
    await api.relink('resource',language);expect(mock.invoke).toHaveBeenLastCalledWith('pick_relink',{id:'resource',language});
    await api.backup(language);expect(mock.invoke).toHaveBeenLastCalledWith('backup',{language});
    await api.restore(language);expect(mock.invoke).toHaveBeenLastCalledWith('restore',{language});
    await api.importDrop('native-token','workspace',true,language);expect(mock.invoke).toHaveBeenLastCalledWith('import_drop',{token:'native-token',workspaceId:'workspace',authorize:true,language});
  }
  expect(mock.invoke).not.toHaveBeenCalledWith('begin_search_session');
});

it('常用位置只作为选择器起点传递，未传入时明确选择任意目录',async()=>{
  mock.invoke.mockResolvedValue(null);
  const {api}=await import('./api');
  for(const suggestion of ['desktop','downloads','documents'] as const){
    expect(await api.pickRoot('en',suggestion)).toBeNull();
    expect(mock.invoke).toHaveBeenLastCalledWith('pick_root',{language:'en',suggestion});
  }
  await api.pickRoot('zh');expect(mock.invoke).toHaveBeenLastCalledWith('pick_root',{language:'zh',suggestion:null});
  expect(mock.invoke).not.toHaveBeenCalledWith('mutate',expect.anything());
});

it('错误在显示时选择当前语言，逐项空文案和非标准拒绝仍给出可读反馈',async()=>{
  const {errorMessage}=await import('./api');
  const error={code:'not_authorized',message:'文件未授权',message_en:'The file is not authorized.'};
  expect(errorMessage(error,'zh')).toBe('文件未授权');
  expect(errorMessage(error,'en')).toBe('The file is not authorized.');
  expect(errorMessage(new Error('External plugin failure'),'en')).toBe('External plugin failure');
  expect(errorMessage('External string failure','zh')).toBe('External string failure');
  for(const cause of [null,false,undefined,'',{message:null,message_en:null},{message:42}]){
    expect(errorMessage(cause,'zh')).toBe('操作失败，请重试');
    expect(errorMessage(cause,'en')).toBe('Operation failed. Please try again.');
  }
});
