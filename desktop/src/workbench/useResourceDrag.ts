import { useEffect, useRef, useState } from 'react';

// Pointer Events 与系统文件拖放分开；只传资源 UUID，不把 DOM 路径当授权。
export function useResourceDrag(onDrop:(ids:string[],workspace:string)=>void) {
  const latest=useRef(onDrop);latest.current=onDrop;
  const [drag,setDrag]=useState<{x:number;y:number;count:number}|null>(null);
  const ignoreClick=useRef(false);
  const pending=useRef<{x:number;y:number;ids:string[];active:boolean}|null>(null);
  useEffect(()=>{
    const clear=()=>{pending.current=null;setDrag(null);document.querySelectorAll('.drop-target').forEach(e=>e.classList.remove('drop-target'));};
    const move=(event:PointerEvent)=>{
      const p=pending.current;if(!p)return;
      if(!p.active&&Math.hypot(event.clientX-p.x,event.clientY-p.y)<7)return;
      p.active=true;setDrag({x:event.clientX,y:event.clientY,count:p.ids.length});
      document.querySelectorAll('.drop-target').forEach(e=>e.classList.remove('drop-target'));
      document.elementFromPoint(event.clientX,event.clientY)?.closest('[data-workspace-id]')?.classList.add('drop-target');
    };
    const up=(event:PointerEvent)=>{
      const p=pending.current;
      if(p?.active){ignoreClick.current=true;window.setTimeout(()=>{ignoreClick.current=false;},300);const target=document.elementFromPoint(event.clientX,event.clientY)?.closest<HTMLElement>('[data-workspace-id]');if(target?.dataset.workspaceId)latest.current(p.ids,target.dataset.workspaceId);}
      clear();
    };
    const key=(event:KeyboardEvent)=>{if(event.key==='Escape')clear();};
    const click=(event:MouseEvent)=>{if(ignoreClick.current){event.preventDefault();event.stopPropagation();ignoreClick.current=false;}};
    window.addEventListener('keydown',key);window.addEventListener('blur',clear);
    window.addEventListener('click',click,true);
    window.addEventListener('pointermove',move);window.addEventListener('pointerup',up);window.addEventListener('pointercancel',clear);
    return()=>{window.removeEventListener('keydown',key);window.removeEventListener('blur',clear);window.removeEventListener('click',click,true);window.removeEventListener('pointermove',move);window.removeEventListener('pointerup',up);window.removeEventListener('pointercancel',clear);};
  },[]);
  return {drag,onStart:(event:React.PointerEvent,ids:string[])=>{
    if(event.button!==0||(event.target as HTMLElement).closest('.row-actions,input'))return;
    // WebKit默认文字选取/拖动会取消Pointer手势；只阻止资料拖动起点，不影响复选框和操作按钮。
    event.preventDefault();
    pending.current={x:event.clientX,y:event.clientY,ids,active:false};
  }};
}
