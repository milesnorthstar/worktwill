import { invoke, isTauri } from '@tauri-apps/api/core';
import type { Mutation, MutationResponse, Overview, ResourcePage, ResourceQuery, SearchRequest, SearchResponse, ImportResult, TimelineRequest, TimelinePage, SearchPageRequest, WebMetadata, UiLanguage, SuggestedLocation } from './contracts';

export const native = isTauri();
let searchSession:Promise<string>|null=null;
let searchRequestId=0;
export function nextSearchRequestId():number {
  if(searchRequestId===0xffffffff)throw {message:'搜索会话计数已满，请重启应用',message_en:'Restart the app to renew search'};
  return ++searchRequestId;
}
function currentSearchSession():Promise<string> {
  if(!searchSession){
    const pending=invoke<string>('begin_search_session');searchSession=pending;
    // 初始连接失败仍向调用者报错；下一次明确搜索允许重新建立查询会话。
    void pending.catch(()=>{if(searchSession===pending)searchSession=null;});
  }
  return searchSession;
}
export const api = {
  searchPage: async(request:SearchPageRequest) => invoke<SearchResponse>('search_page',{request,session:await currentSearchSession()}),
  cancelSearch: async(lane:'quick'|'page',requestId:number) => {
    const active=searchSession;
    if(!active)return false;
    return invoke<boolean>('cancel_search',{lane,requestId,session:await active});
  },
  timeline: (request: TimelineRequest) => invoke<TimelinePage>('timeline', {request}),
  webMetadata: (url:string) => invoke<WebMetadata>('fetch_web_metadata',{url}),
  importDrop: (token:string,workspace_id:string|null,authorize:boolean,language:UiLanguage) => invoke<ImportResult>('import_drop',{token,workspaceId:workspace_id,authorize,language}),
  cancelDrop: (token:string) => invoke<void>('cancel_drop',{token}),
  pickFiles: (workspace_id:string|null,language:UiLanguage) => invoke<ImportResult|null>('pick_files',{workspaceId:workspace_id,language}),
  overview: () => invoke<Overview>('overview'),
  resources: (query: ResourceQuery) => invoke<ResourcePage>('resources', { query }),
  search: async(request: SearchRequest) => invoke<SearchResponse>('search', { request,session:await currentSearchSession() }),
  mutate: (mutation: Mutation) => invoke<MutationResponse>('mutate', {
    request: { operation_id: crypto.randomUUID(), mutation },
  }),
  pickRoot: (language:UiLanguage,suggestion:SuggestedLocation|null=null) => invoke<string | null>('pick_root',{language,suggestion}),
  rescan: () => invoke<void>('rescan'),
  open: (id: string) => invoke<void>('open_resource', { id }),
  reveal: (id: string) => invoke<void>('reveal_resource', { id }),
  relink: (id: string,language:UiLanguage) => invoke<boolean>('pick_relink', { id,language }),
  backup: (language:UiLanguage) => invoke<boolean>('backup',{language}),
  restore: (language:UiLanguage) => invoke<boolean>('restore',{language}),
};

export function errorMessage(error: unknown, language: UiLanguage): string {
  if (typeof error === 'object' && error !== null) {
    if (language === 'en' && 'message_en' in error && typeof error.message_en === 'string' && error.message_en) return error.message_en;
    if ('message' in error && typeof error.message === 'string' && error.message) return error.message;
  }
  return typeof error === 'string' && error ? error : language === 'zh' ? '操作失败，请重试' : 'Operation failed. Please try again.';
}
