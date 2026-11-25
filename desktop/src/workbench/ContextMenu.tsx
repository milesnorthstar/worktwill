import { useEffect, useLayoutEffect, useRef } from 'react';
import type { KeyboardEvent, MouseEvent } from 'react';

export type ContextMenuEvent = MouseEvent<HTMLElement> | KeyboardEvent<HTMLElement>;
export type ContextMenuAnchor = { x: number; y: number; trigger: HTMLElement };
export type ContextMenuItem = { label: string; run: () => void; disabled?: boolean; destructive?: boolean };

export function isContextMenuKey(event: KeyboardEvent<HTMLElement>) {
  return !event.repeat && (event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10'));
}

export function contextMenuAnchor(event: ContextMenuEvent): ContextMenuAnchor {
  const row = event.currentTarget;
  const control = (event.target as HTMLElement).closest<HTMLElement>('button:enabled, input:enabled');
  const trigger = control && row.contains(control) ? control :
    row.querySelector<HTMLElement>('.resource-name:enabled') ??
    row.querySelector<HTMLElement>('.resource-copy input:enabled') ?? row;
  if ('key' in event) {
    const bounds = trigger.getBoundingClientRect();
    return { x: bounds.left, y: bounds.bottom, trigger };
  }
  return { x: event.clientX, y: event.clientY, trigger };
}

export default function ContextMenu({ anchor, label, items, onClose }: {
  anchor: ContextMenuAnchor; label: string; items: ContextMenuItem[]; onClose: (restoreFocus?: boolean) => void;
}) {
  const menu = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    const element = menu.current!;
    const { width, height } = element.getBoundingClientRect();
    const margin = 8;
    element.style.left = Math.max(margin, Math.min(anchor.x, window.innerWidth - width - margin)) + 'px';
    element.style.top = Math.max(margin, Math.min(anchor.y, window.innerHeight - height - margin)) + 'px';
    (element.querySelector<HTMLElement>('button:enabled') ?? element).focus({ preventScroll: true });
  }, [anchor, label]);

  useEffect(() => {
    function outside(event: Event) {
      if (!menu.current?.contains(event.target as Node)) onClose(false);
    }
    function scroll(event: Event) {
      if (!menu.current?.contains(event.target as Node)) onClose();
    }
    function dismiss() { onClose(); }
    function blur() { onClose(false); }
    window.addEventListener('pointerdown', outside, true);
    window.addEventListener('scroll', scroll, true);
    window.addEventListener('resize', dismiss);
    window.addEventListener('blur', blur);
    return () => {
      window.removeEventListener('pointerdown', outside, true);
      window.removeEventListener('scroll', scroll, true);
      window.removeEventListener('resize', dismiss);
      window.removeEventListener('blur', blur);
    };
  }, [onClose]);

  function moveFocus(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === 'Escape' || event.key === 'Tab') {
      event.stopPropagation();
      if (event.key === 'Escape') event.preventDefault();
      onClose();
      return;
    }
    if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return;
    event.preventDefault();
    event.stopPropagation();
    const buttons = Array.from(event.currentTarget.querySelectorAll<HTMLButtonElement>('button:enabled'));
    const index = buttons.findIndex(button => button === document.activeElement);
    const last = buttons.length - 1;
    const next = event.key === 'Home' ? 0 : event.key === 'End' ? last :
      event.key === 'ArrowDown' ? (index === last ? 0 : index + 1) : (index <= 0 ? last : index - 1);
    buttons[next]?.focus();
  }

  return <div ref={menu} className="context-menu" role="menu" aria-label={label} tabIndex={-1}
    onKeyDown={moveFocus} onContextMenu={event => event.preventDefault()}>
    {items.map(item => <button key={item.label} type="button" role="menuitem" tabIndex={-1} disabled={item.disabled}
      className={item.destructive ? 'context-menu-danger' : undefined} onClick={() => { onClose(); item.run(); }}>
      {item.label}
    </button>)}
  </div>;
}
