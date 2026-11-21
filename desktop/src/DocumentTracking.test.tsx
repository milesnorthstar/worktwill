import {render,screen} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {expect,it,vi} from 'vitest';
const mocks=vi.hoisted(()=>({documentTracking:vi.fn(),setDocumentTracking:vi.fn()}));
vi.mock('./api',async original=>({...await original<typeof import('./api')>(),api:mocks}));
import DocumentTracking from './DocumentTracking';

it('目录授权后仍需明确同意，取消不写元数据，失败结果不显示成功',async()=>{
  mocks.documentTracking.mockResolvedValue({supported:true,roots:[],files:[{resource_id:'f',name:'合同.docx',established:false,error:{code:'denied',message:'无法建立标记',message_en:'Unable to establish marker'}}]});
  mocks.setDocumentTracking.mockResolvedValue(undefined);
  const user=userEvent.setup();
  render(<DocumentTracking roots={[{id:'root',path:'C:\\fixture',active:true,paused:false,scan_state:'completed',location_status:'available',scanned:1,failures:0,exclusions:[]}]} revision={1} language="zh" busy={false} run={task=>task()}/>);
  await user.click(await screen.findByRole('button',{name:'启用保存追踪'}));
  expect(mocks.setDocumentTracking).not.toHaveBeenCalled();
  await user.click(screen.getByRole('button',{name:'取消'}));
  expect(mocks.setDocumentTracking).not.toHaveBeenCalled();
  expect(screen.queryByText('追踪标记已建立')).not.toBeInTheDocument();
  await user.click(screen.getByRole('button',{name:'启用保存追踪'}));
  await user.click(screen.getByRole('button',{name:'同意并启用追踪'}));
  expect(mocks.setDocumentTracking).toHaveBeenCalledExactlyOnceWith('root',true);
});

it('暂停位置仍可关闭追踪同意',async()=>{
  mocks.setDocumentTracking.mockClear();
  mocks.documentTracking.mockResolvedValue({supported:true,roots:[{root_id:'paused',enabled:true}],files:[]});
  const user=userEvent.setup();
  render(<DocumentTracking roots={[{id:'paused',path:'C:\\offline',active:true,paused:true,scan_state:'idle',location_status:'unavailable',scanned:0,failures:0,exclusions:[]}]} revision={1} language="zh" busy={false} run={task=>task()}/>);
  const disable=await screen.findByRole('button',{name:'关闭追踪'});
  expect(disable).toBeEnabled();
  await user.click(disable);
  expect(mocks.setDocumentTracking).toHaveBeenCalledExactlyOnceWith('paused',false);
});
