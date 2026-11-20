// 仅开发服务器的视觉夹具：合成数据与 mock IPC，不可作为原生端到端证据。
import {createRoot} from 'react-dom/client';
import {mockIPC} from '@tauri-apps/api/mocks';
import type {Resource, Root, TimelineEntry, TimelineRequest} from '../contracts';
import '../styles.css';

Reflect.set(globalThis,'isTauri',true);
const now=new Date();
const emptyReview=new URLSearchParams(window.location.search).has('empty');
const failureReview=new URLSearchParams(window.location.search).get('failure');
const singleFilesReview=new URLSearchParams(window.location.search).get('scope')==='files-only';
const rootsReview=new URLSearchParams(window.location.search).get('scope')==='root-locations';
const reviewRoots:Root[]=rootsReview?[
  {id:'r',path:'/synthetic/Department-'+ 'B'.repeat(180)+'/Managed-documents',active:true,paused:false,scan_state:'completed',location_status:'available',scanned:120,failures:0,exclusions:[]},
  {id:'legacy',path:'/synthetic/Legacy-documents',active:true,paused:true,scan_state:'paused',location_status:'legacy',scanned:50,failures:0,exclusions:[]},
  {id:'unavailable',path:'/synthetic/Folder-moved-while-paused',active:true,paused:true,scan_state:'paused',location_status:'unavailable',scanned:30,failures:0,exclusions:['private']},
  {id:'revoked',path:'/synthetic/Revoked-documents',active:false,paused:false,scan_state:'cancelled',location_status:'unavailable',scanned:10,failures:0,exclusions:[]},
]:[{id:'r',path:'/验收合成资料',active:true,paused:false,scan_state:'completed',location_status:'available',scanned:4,failures:0,exclusions:[]}];
const at=(dayOffset:number,hour:number,minute:number)=>new Date(now.getFullYear(),now.getMonth(),now.getDate()-dayOffset,hour,minute).getTime();
let revision=1;
let workspaces=[{id:'w1',name:'张三入职',count:2},{id:'w2',name:'2026 招聘',count:2},{id:'w3',name:'合同管理',count:1}];
const resource=(id:string,name:string,extension:Partial<Resource>={}):Resource=>({id,name,kind:'file',location:'/验收合成资料/'+name,status:'available',modified_at:at(0,10,24),size:10240,favorite:false,workspace_ids:['w1'],...extension});
let resources=[resource('f1','劳动合同-张三.docx',{favorite:true,workspace_ids:['w1','w3']}),resource('f2','入职登记表.xlsx'),resource('f3','2026招聘计划.pdf',{workspace_ids:['w2']}),resource('u1','招聘后台',{kind:'web',location:'https://example.com/jobs',workspace_ids:['w2']}),resource('u2','合同模板网站',{kind:'web',location:'https://example.com/contracts',workspace_ids:['w3']}),resource('f4','项目合作协议.docx',{modified_at:at(1,17,30),workspace_ids:['w3']})];
const entry=(id:string,kind:string,r:Resource,time:number):TimelineEntry=>({activity:{id,kind,name:r.name,resource_id:r.id,at:time+60000,source:kind==='opened'?'app':'filesystem',file_modified_at:kind==='opened'?null:time},resource:r,effective_at:kind==='file_discovered'?time+60000:time});
let events=[entry('e1','file_modified',resources[0],at(0,10,24)),entry('e2','opened',resources[1],at(0,9,40)),entry('e3','file_discovered',resources[2],at(0,8,55)),entry('e4','file_modified',resources[5],at(1,17,30))];
const fileGrants=[resource('grant-long','Employment contract — signed original and supporting attachments.docx',{workspace_ids:[],location:'/synthetic/Department-'+ 'A'.repeat(180)+'/Employment-contract.docx'})];
if(singleFilesReview){
  fileGrants.push(resource('grant-missing','出差报销材料.pdf',{status:'missing',workspace_ids:[]}),resource('grant-unconfirmed','客户合作协议.docx',{status:'identity_unconfirmed',workspace_ids:[]}));
  resources=[...fileGrants];workspaces=[];events=[];
}
if(emptyReview){workspaces=[];resources=[];events=[];}
const page=(items:Resource[])=>({session:'visual-fixture',permission_version:1,revision,items,has_more:false});
mockIPC((command,args)=>{
  const input=args as Record<string,any>;
  if(failureReview==='web'&&(command==='fetch_web_metadata'||command==='mutate'))return Promise.reject({message:command==='mutate'?'本地数据操作失败，请重试或检查备份':'网站暂不可用，可手动填写标题',message_en:command==='mutate'?'The local data operation failed. Try again or check your backup.':'The website is unavailable. Enter the title manually.'});
  if(failureReview==='timeline'&&command==='timeline')return Promise.reject({message:'本地数据操作失败，请重试或检查备份',message_en:'The local data operation failed. Try again or check your backup.'});
  switch(command){
    case 'begin_search_session':return 'visual-query-session';
    case 'cancel_search':return true;
    case 'overview':return {session:'visual-fixture',permission_version:1,revision,sqlite_version:'visual-fixture',roots:emptyReview||singleFilesReview?[]:reviewRoots,file_grants:emptyReview?[]:fileGrants,workspaces,activities:events.map(e=>e.activity)};
    case 'resources':return page(resources.filter(r=>(!input.query.workspace_id||r.workspace_ids.includes(input.query.workspace_id))&&(input.query.kind==='all'||r.kind===input.query.kind)&&(input.query.mode!=='favorites'||r.favorite)));
    case 'search':case 'search_page':{const req=input.request.request??input.request;return {request_id:req.request_id,page:page([...resources,...workspaces.map(w=>resource(w.id,w.name,{kind:'workspace',location:'',workspace_ids:[]}))].filter(r=>(r.name+r.location).includes(req.query))) };}
    case 'timeline':{const q=input.request as TimelineRequest;const matching=events.filter(e=>(q.filter==='all'||(q.filter==='modified'?e.activity.kind==='file_modified':q.filter==='discovered'?e.activity.kind==='file_discovered':q.filter==='opened'?e.activity.kind==='opened':false))&&(!q.workspace_id||e.resource?.workspace_ids.includes(q.workspace_id)));return {session:'visual-fixture',permission_version:1,revision,events:matching.filter(e=>(q.from===null||e.effective_at>=q.from)&&(q.to===null||e.effective_at<q.to)),calendar:q.day_boundaries.slice(0,-1).map((at,index)=>({at,count:matching.filter(e=>e.effective_at>=at&&e.effective_at<q.day_boundaries[index+1]).length})).filter(point=>point.count>0),has_more:false};}
    case 'mutate':{const m=input.request.mutation;revision++;let id=m.id??crypto.randomUUID();if(m.type==='create_workspace'){const ids=m.resource_ids??[];workspaces=[{id,name:m.name,count:resources.filter(resource=>ids.includes(resource.id)).length},...workspaces];resources=resources.map(resource=>ids.includes(resource.id)?{...resource,workspace_ids:[...resource.workspace_ids,id]}:resource);}if(m.type==='rename_workspace')workspaces=workspaces.map(w=>w.id===m.id?{...w,name:m.name}:w);if(m.type==='delete_workspace')workspaces=workspaces.filter(w=>w.id!==m.id);if(m.type==='save_web')resources=[resource(id,m.name,{kind:'web',location:m.url,workspace_ids:m.workspace_id?[m.workspace_id]:[]}),...resources];return {id,changed:1};}
    case 'fetch_web_metadata':return {title:'Example Domain',final_url:input.url};
    case 'plugin:event|listen':return 1;
    case 'plugin:event|unlisten':return;
    default:return null;
  }
});
const {default:App}=await import('../App');
const banner=document.createElement('div');banner.textContent='视觉验收夹具 · 合成资料 · 不连接原生后端'+(failureReview?' · 模拟失败':'');banner.style.cssText='position:fixed;bottom:10px;left:14px;z-index:100;font-size:10px;color:#8a97ad;background:#ffffffdd;padding:4px 8px;border:1px solid #e5eaf2;border-radius:5px;pointer-events:none';document.body.append(banner);
createRoot(document.getElementById('root')!).render(<App/>);
