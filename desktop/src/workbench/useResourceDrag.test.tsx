import {fireEvent,render,screen} from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import {expect,it,vi} from 'vitest';
import {useResourceDrag} from './useResourceDrag';
it('内部拖拽只携带UUID，在目标工作区落下后才关联',()=>{
  vi.stubGlobal('PointerEvent',MouseEvent);
  const dropped=vi.fn();
  function Fixture(){const{drag,onStart}=useResourceDrag(dropped);return <><div onPointerDown={event=>onStart(event,['resource-uuid'])}>资料</div><button data-workspace-id="target-workspace">目标工作区</button>{drag&&<span>正在拖拽</span>}</>;}
  render(<Fixture/>);const target=screen.getByRole('button');
  Object.defineProperty(document,'elementFromPoint',{configurable:true,value:vi.fn(()=>target)});
  fireEvent.pointerDown(screen.getByText('资料'),{button:0,clientX:10,clientY:10});
  fireEvent.pointerMove(window,{clientX:12,clientY:12});expect(screen.queryByText('正在拖拽')).not.toBeInTheDocument();
  fireEvent.pointerMove(window,{clientX:100,clientY:100});expect(screen.getByText('正在拖拽')).toBeVisible();expect(dropped).not.toHaveBeenCalled();
  fireEvent.pointerUp(window,{clientX:100,clientY:100});expect(dropped).toHaveBeenCalledExactlyOnceWith(['resource-uuid'],'target-workspace');
  expect(screen.queryByText('正在拖拽')).not.toBeInTheDocument();vi.unstubAllGlobals();
});

it.each(['escape','blur'])('拖拽%s取消后松开不建立关联',method=>{
  vi.stubGlobal('PointerEvent',MouseEvent);const dropped=vi.fn();
  function Fixture(){const{onStart}=useResourceDrag(dropped);return <><div onPointerDown={event=>onStart(event,['file'])}>文件</div><button data-workspace-id="w">目标</button></>;}
  render(<Fixture/>);Object.defineProperty(document,'elementFromPoint',{configurable:true,value:()=>screen.getByRole('button')});
  fireEvent.pointerDown(screen.getByText('文件'),{button:0,clientX:0,clientY:0});fireEvent.pointerMove(window,{clientX:100,clientY:100});
  if(method==='escape')fireEvent.keyDown(window,{key:'Escape'});else fireEvent.blur(window);
  fireEvent.pointerUp(window,{clientX:100,clientY:100});expect(dropped).not.toHaveBeenCalled();vi.unstubAllGlobals();
});

it('原生手势抑制后仍可单击选择，拖动松手不误选或重复加入',async()=>{
  vi.stubGlobal('PointerEvent',MouseEvent);
  const dropped=vi.fn(),selected=vi.fn();
  function Fixture(){const{onStart}=useResourceDrag(dropped);return <><button onPointerDown={event=>onStart(event,['file'])} onClick={selected}>合同</button><button data-workspace-id="w">工作区</button></>;}
  render(<Fixture/>);const user=userEvent.setup(),source=screen.getByRole('button',{name:'合同'}),target=screen.getByRole('button',{name:'工作区'});
  Object.defineProperty(document,'elementFromPoint',{configurable:true,value:()=>target});
  await user.click(source);expect(selected).toHaveBeenCalledTimes(1);expect(dropped).not.toHaveBeenCalled();
  await user.pointer([{keys:'[MouseLeft>]',target:source,coords:{clientX:10,clientY:10}},{target:source,coords:{clientX:100,clientY:100}},{keys:'[/MouseLeft]',target:source,coords:{clientX:100,clientY:100}}]);
  expect(dropped).toHaveBeenCalledExactlyOnceWith(['file'],'w');expect(selected).toHaveBeenCalledTimes(1);
  vi.unstubAllGlobals();
});
