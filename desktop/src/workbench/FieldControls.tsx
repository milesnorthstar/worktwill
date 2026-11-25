import { useCallback, useId, useLayoutEffect, useRef, useState } from 'react';
import type { KeyboardEvent, ReactNode, RefObject } from 'react';
import { createPortal } from 'react-dom';
import { CalendarDays, Check, ChevronDown, ChevronLeft, ChevronRight } from 'lucide-react';
import './fields.css';

type Language = 'zh' | 'en';
type Option<Value extends string> = {value: Value; label: string};

function FieldPanel({ anchor, id, role, label, onClose, children }: {
  anchor: RefObject<HTMLElement | null>; id: string; role: 'listbox' | 'dialog';
  label: string; onClose: (focus?: boolean) => void; children: ReactNode;
}) {
  const panel = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const control = anchor.current;
    const surface = panel.current;
    if (!control || !surface) return;
    function place() {
      if (!control || !surface) return;
      const {left, top, width, bottom} = control.getBoundingClientRect();
      const panelWidth = Math.min(window.innerWidth - 24, Math.max(width, role === 'dialog' ? 302 : 176));
      const roomBelow = window.innerHeight - bottom - 18;
      const roomAbove = top - 18;
      surface.style.width = panelWidth + 'px';
      const maxHeight = role === 'dialog' ? 372 : 336;
      const idealHeight = Math.min(surface.scrollHeight + 2, maxHeight);
      const above = roomBelow < idealHeight && roomAbove > roomBelow;
      surface.style.left = Math.max(12, Math.min(left, window.innerWidth - panelWidth - 12)) + 'px';
      surface.style.maxHeight = Math.max(40, Math.min(maxHeight, above ? roomAbove : roomBelow)) + 'px';
      surface.style.top = Math.max(12, Math.min(
        above ? top - surface.offsetHeight - 6 : bottom + 6,
        window.innerHeight - surface.offsetHeight - 12,
      )) + 'px';
    }
    // 随页面滚动重新定位，弹层内滚动时保留位置。
    place();
    function outside(event: Event) {
      if (!control?.contains(event.target as Node) && !surface?.contains(event.target as Node)) onClose(false);
    }
    function scroll(event: Event) {
      if (!surface?.contains(event.target as Node)) place();
    }
    document.addEventListener('pointerdown', outside, true);
    document.addEventListener('focusin', outside);
    window.addEventListener('scroll', scroll, true);
    window.addEventListener('resize', place);
    return () => {
      document.removeEventListener('pointerdown', outside, true);
      document.removeEventListener('focusin', outside);
      window.removeEventListener('scroll', scroll, true);
      window.removeEventListener('resize', place);
    };
  }, [anchor, role, onClose]);
  const content = <div ref={panel} id={id} role={role} aria-label={label} className="field-panel"
    onKeyDown={event => {
      if (event.key === 'Escape') {
        event.preventDefault();
        event.stopPropagation();
        onClose();
      }
    }}>{children}</div>;
  // 日期弹层留在输入框旁的 DOM 顺序里，Tab 可以继续去下一个表单项。
  return role === 'dialog' ? content : createPortal(content, anchor.current?.closest('dialog') ?? document.body);
}

export function ChoiceField<Value extends string>({ label, value, options, onChange, placeholder, required, autoFocus }: {
  label: string; value: Value; options: Option<Value>[]; onChange: (value: Value) => void;
  placeholder?: string; required?: boolean; autoFocus?: boolean;
}) {
  const id = useId();
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const button = useRef<HTMLButtonElement>(null);
  const typeAhead = useRef({text: '', at: 0});
  const selected = options.findIndex(option => option.value === value);
  const close = useCallback((focus = true) => {
    setOpen(false);
    if (focus) button.current?.focus({preventScroll: true});
  }, []);
  function select(index: number) {
    const option = options[index];
    if (option) onChange(option.value);
    close();
  }
  useLayoutEffect(() => {
    if (!open) return;
    const list = document.getElementById(id);
    const item = document.getElementById(id + '-' + active);
    if (!list || !item) return;
    const itemTop = item.offsetTop;
    const itemBottom = itemTop + item.offsetHeight;
    if (itemTop < list.scrollTop) list.scrollTop = itemTop;
    if (itemBottom > list.scrollTop + list.clientHeight) list.scrollTop = itemBottom - list.clientHeight;
  }, [active, id, open]);
  function keyboard(event: KeyboardEvent<HTMLButtonElement>) {
    if (event.key === 'Escape' && open) {
      event.preventDefault();
      event.stopPropagation();
      close();
    } else if (event.key === 'Tab' && open) {
      close(false);
    } else if (['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) {
      event.preventDefault();
      const last = options.length - 1;
      const next = event.key === 'Home' ? 0 : event.key === 'End' ? last :
        open ? active + (event.key === 'ArrowDown' ? 1 : -1) :
          selected < 0 && event.key === 'ArrowUp' ? last : Math.max(0, selected);
      setActive(Math.max(0, Math.min(last, next)));
      setOpen(true);
    } else if (open && (event.key === 'Enter' || event.key === ' ')) {
      event.preventDefault();
      select(active);
    } else if (event.key.length === 1 && event.key.trim() && !event.ctrlKey && !event.metaKey && !event.altKey) {
      const now = Date.now();
      const prefix = (now - typeAhead.current.at < 650 ? typeAhead.current.text : '') + event.key.toLocaleLowerCase();
      typeAhead.current = {text: prefix, at: now};
      const match = options.findIndex(option => option.label.toLocaleLowerCase().startsWith(prefix));
      if (match >= 0) {
        event.preventDefault();
        setActive(match);
        setOpen(true);
      }
    }
  }
  return <div className="choice-field">
    <span id={id + '-label'} className="field-label">{label}</span>
    <button ref={button} type="button" role="combobox" className="field-trigger"
      data-field-autofocus={autoFocus || undefined} aria-expanded={open} aria-required={required}
      aria-labelledby={id + '-label'} aria-controls={open ? id : undefined}
      aria-activedescendant={open && options[active] ? id + '-' + active : undefined}
      disabled={!options.length} onKeyDown={keyboard}
      onClick={() => {
        if (open) close();
        else {
          button.current?.focus({preventScroll: true});
          setActive(Math.max(0, selected));
          setOpen(true);
        }
      }}>
      <span className={selected < 0 ? 'field-placeholder' : undefined}>{options[selected]?.label ?? placeholder ?? label}</span>
      <ChevronDown size={16} aria-hidden="true" />
    </button>
    {open && <FieldPanel anchor={button} id={id} label={label} role="listbox" onClose={close}>
      {options.map((option, index) => <div key={option.value} id={id + '-' + index} role="option"
        aria-selected={option.value === value} data-active={active === index} className="field-option"
        onMouseDown={event => event.preventDefault()} onMouseMove={() => setActive(index)} onClick={() => select(index)}>
        <span>{option.label}</span>{option.value === value && <Check size={15} aria-hidden="true" />}
      </div>)}
    </FieldPanel>}
  </div>;
}

export function dayKey(date: Date) {
  return [String(date.getFullYear()).padStart(4, '0'), String(date.getMonth() + 1).padStart(2, '0'), String(date.getDate()).padStart(2, '0')].join('-');
}
export function localDay(value: string) {
  const [year, month, day] = value.split('-').map(Number);
  return calendarDate(year, month - 1, day);
}
export function calendarDate(year: number, month: number, day: number) {
  const date = new Date(0);
  date.setHours(0, 0, 0, 0);
  date.setFullYear(year, month, day);
  return date;
}
function monthStart(date: Date) {
  return calendarDate(date.getFullYear(), date.getMonth(), 1);
}
export function shiftMonth(date: Date, offset: number) {
  return calendarDate(date.getFullYear(), date.getMonth() + offset, 1);
}
function validDate(value: string) {
  return /^\d{4}-\d{2}-\d{2}$/.test(value) && value >= '0001-01-01' && dayKey(localDay(value)) === value;
}

export function MonthCalendar({ lang, month, selected, counts, min, max, onSelect, onMonth }: {
  lang: Language; month: Date; selected: string | null; counts?: ReadonlyMap<string, number>;
  min?: string; max?: string; onSelect: (key: string) => void; onMonth: (month: Date) => void;
}) {
  const zh = lang === 'zh';
  const locale = zh ? 'zh-CN' : 'en-US';
  const grid = useRef<HTMLDivElement>(null);
  const focusNext = useRef<string | null>(null);
  const [focused, setFocused] = useState<string | null>(null);
  const first = monthStart(month);
  const nextMonth = shiftMonth(first, 1);
  const end = new Date(nextMonth.getTime() - 1).getDate();
  const leading = (first.getDay() + 6) % 7;
  const today = dayKey(new Date());
  const monthKey = dayKey(first);
  const previousAllowed = monthKey > (min ?? '0001-01-01').slice(0, 7) + '-01';
  const nextAllowed = nextMonth.getFullYear() <= 9999 && dayKey(nextMonth) <= (max ?? '9999-12-31');
  const dates = Array.from({length: end}, (_, index) => {
    const date = new Date(first);
    date.setDate(index + 1);
    return date;
  });
  const enabled = (key: string) => key.length === 10 && key >= (min ?? '0001-01-01') && key <= (max ?? '9999-12-31');
  const tabDay = [focused, selected, today, ...dates.map(dayKey)].find(key => key && key.slice(0, 7) === monthKey.slice(0, 7) && enabled(key));
  useLayoutEffect(() => {
    if (!focusNext.current) return;
    grid.current?.querySelector<HTMLButtonElement>('[data-day="' + focusNext.current + '"]')?.focus();
    focusNext.current = null;
  }, [month]);
  function handleDayKey(event: KeyboardEvent<HTMLButtonElement>, date: Date) {
    const deltas: Record<string, number> = {ArrowLeft: -1, ArrowRight: 1, ArrowUp: -7, ArrowDown: 7};
    const delta = deltas[event.key];
    if (delta === undefined) return;
    event.preventDefault();
    const target = calendarDate(date.getFullYear(), date.getMonth(), date.getDate() + delta);
    const key = dayKey(target);
    if (!enabled(key)) return;
    const currentMonthDay = grid.current?.querySelector<HTMLButtonElement>('[data-day="' + key + '"]');
    if (currentMonthDay) currentMonthDay.focus();
    else {
      focusNext.current = key;
      onMonth(monthStart(target));
    }
  }
  return <div className="month-calendar">
    <div className="calendar-heading">
      <button type="button" className="calendar-nav" disabled={!previousAllowed} aria-label={zh ? '上个月' : 'Previous month'} onClick={() => onMonth(shiftMonth(first, -1))}><ChevronLeft size={17} /></button>
      <h2 aria-live="polite">{first.toLocaleDateString(locale, {year: 'numeric', month: 'long'})}</h2>
      <button type="button" className="calendar-nav" disabled={!nextAllowed} aria-label={zh ? '下个月' : 'Next month'} onClick={() => onMonth(nextMonth)}><ChevronRight size={17} /></button>
    </div>
    <div className="calendar-weekdays" aria-hidden="true">{(zh ? ['一','二','三','四','五','六','日'] : ['Mo','Tu','We','Th','Fr','Sa','Su']).map(day => <span key={day}>{day}</span>)}</div>
    <div ref={grid} className="calendar-days" role="group" aria-label={zh ? '选择日期' : 'Choose date'}>
      {Array.from({length: leading}, (_, index) => <span key={index} aria-hidden="true" />)}
      {dates.map(date => {
        const key = dayKey(date);
        const count = counts?.get(key) ?? 0;
        const countLabel = count ? (zh ? '，' + count + ' 条记录' : ', ' + count + ' activities') : '';
        return <button key={key} type="button" data-day={key} disabled={!enabled(key)} aria-pressed={selected === key}
          aria-current={today === key ? 'date' : undefined} tabIndex={key === tabDay ? 0 : -1}
          aria-label={date.toLocaleDateString(locale, {year: 'numeric', month: 'long', day: 'numeric'}) + countLabel}
          title={count ? countLabel.slice(1).trim() : undefined}
          onFocus={() => setFocused(key)} onClick={() => onSelect(key)} onKeyDown={event => handleDayKey(event, date)}>
          {date.getDate()}{count > 0 && <i aria-hidden="true" />}
        </button>;
      })}
    </div>
  </div>;
}

export function DateField({ label, value, onChange, min, max, lang }: {
  label: string; value: string; onChange: (value: string) => void; min?: string; max?: string; lang: Language;
}) {
  const id = useId();
  const [open, setOpen] = useState(false);
  const [month, setMonth] = useState(() => monthStart(new Date()));
  const [draft, setDraft] = useState<string | null>(null);
  const button = useRef<HTMLButtonElement>(null);
  const control = useRef<HTMLDivElement>(null);
  const zh = lang === 'zh';
  const withinRange = (key: string) => (!min || key >= min) && (!max || key <= max);
  const close = useCallback((focus = true) => {
    setOpen(false);
    if (focus) button.current?.focus({preventScroll: true});
  }, []);
  function show() {
    const today = dayKey(new Date());
    const initial = value && withinRange(value) ? value : min && today < min ? min : max && today > max ? max : today;
    setMonth(monthStart(localDay(initial)));
    setOpen(true);
  }
  function commit(key: string) {
    setDraft(null);
    onChange(key);
    close();
  }
  const displayed = draft ?? value;
  const invalid = displayed !== '' && (!validDate(displayed) || !withinRange(displayed));
  return <div className="date-field">
    <label htmlFor={id}>{label}</label>
    <div ref={control} className="date-field-control">
      <input id={id} type="text" placeholder="YYYY-MM-DD" value={displayed}
        aria-invalid={invalid || undefined}
        onBlur={() => setDraft(null)}
        onChange={event => {
          const next = event.target.value;
          setDraft(next);
          if (!next || (validDate(next) && withinRange(next))) onChange(next);
        }}
        onKeyDown={event => {
          if (event.altKey && event.key === 'ArrowDown') {
            event.preventDefault();
            button.current?.focus();
            show();
          }
        }} />
      <button ref={button} className="date-calendar-trigger" type="button"
        aria-label={zh ? label + '，打开日历' : label + ', open calendar'} aria-haspopup="dialog"
        aria-expanded={open} aria-controls={open ? id + '-calendar' : undefined} onClick={() => {
          if (open) close();
          else {
            button.current?.focus({preventScroll: true});
            show();
          }
        }}
        onKeyDown={event => {
          if (open && event.key === 'Escape') {
            event.preventDefault();
            event.stopPropagation();
            close();
          }
        }}><CalendarDays size={16} aria-hidden="true" /></button>
    </div>
    {open && <FieldPanel anchor={control} id={id + '-calendar'} label={label} role="dialog" onClose={close}>
      <MonthCalendar lang={lang} month={month} selected={value} min={min} max={max} onMonth={setMonth} onSelect={commit} />
      <div className="calendar-footer">
        <button type="button" className="text-button" disabled={!value} onClick={() => commit('')}>{zh ? '清除日期' : 'Clear date'}</button>
        <button type="button" className="text-button" disabled={!withinRange(dayKey(new Date()))} onClick={() => commit(dayKey(new Date()))}>{zh ? '今天' : 'Today'}</button>
      </div>
    </FieldPanel>}
  </div>;
}
