import {File, FileImage, FileText, Film, Folder, Globe, Presentation} from 'lucide-react';
import type {Resource} from '../contracts';

export function ResourceIcon({item}:{item:Resource}) {
  const extension=item.name.split('.').pop()?.toLowerCase()??'';
  const type=item.kind==='web'?'web':item.kind==='workspace'?'workspace':/^(docx?|odt|rtf)$/.test(extension)?'word':/^(xlsx?|csv|ods)$/.test(extension)?'sheet':/^(pptx?|odp)$/.test(extension)?'slides':extension==='pdf'?'pdf':/^(png|jpe?g|gif|webp|svg|heic)$/.test(extension)?'image':/^(mp4|mov|mkv|webm)$/.test(extension)?'video':'file';
  const Icon=type==='web'?Globe:type==='workspace'?Folder:type==='image'?FileImage:type==='video'?Film:type==='slides'?Presentation:type==='pdf'?FileText:File;
  return <span className={`resource-icon ${type}`} aria-hidden="true">{type==='word'||type==='sheet'?<span className="document-glyph"><span>{type==='word'?'W':'X'}</span></span>:<Icon size={22}/>}</span>;
}

export function MatchText({text,query='',enabled=false}:{text:string;query?:string;enabled?:boolean}) {
  if(!enabled||!query.trim())return <>{text}</>;
  // 命中字段由 Rust 决定；这里只标记该字段中的字面命中，不自行判定搜索结果。
  const source=text.normalize('NFC');
  const needle=query.trim().normalize('NFC').toLowerCase();
  const folded=source.toLowerCase();
  const parts:React.ReactNode[]=[];
  let start=0,index=folded.indexOf(needle);
  // 映射大小写展开后的偏移，避免 İ 等字符让后续高亮错位。
  const offsets:number[]=[],ends:number[]=[];let offset=0;
  for(const char of source){for(let i=0;i<char.toLowerCase().length;i++){offsets.push(offset);ends.push(offset+char.length);}offset+=char.length;}
  while(index>=0){const from=offsets[index],to=ends[index+needle.length-1];if(from>=start){parts.push(source.slice(start,from),<mark key={index}>{source.slice(from,to)}</mark>);start=to;}index=folded.indexOf(needle,index+needle.length);}
  parts.push(source.slice(start));
  return <>{parts}</>;
}

export function resourceSize(item:Resource) {
  if(item.kind!=='file'||item.status==='permission_denied')return '';
  const bytes=item.size;
  if(bytes<1024)return `${bytes} B`;
  if(bytes<1024*1024)return `${Math.round(bytes/1024)} KB`;
  if(bytes<1024*1024*1024)return `${Number((bytes/1024/1024).toFixed(1))} MB`;
  return `${Number((bytes/1024/1024/1024).toFixed(1))} GB`;
}

export function resourceTime(timestamp:number|undefined,lang:'zh'|'en') {
  if(!timestamp)return '—';
  const date=new Date(timestamp),today=new Date();
  const yesterday=new Date(today.getFullYear(),today.getMonth(),today.getDate()-1);
  const sameDay=(other:Date)=>date.toDateString()===other.toDateString();
  const time=date.toLocaleTimeString(lang==='zh'?'zh-CN':'en-US',{hour:'2-digit',minute:'2-digit',hour12:false});
  return sameDay(today)?`${lang==='zh'?'今天':'Today'} ${time}`:sameDay(yesterday)?`${lang==='zh'?'昨天':'Yesterday'} ${time}`:date.toLocaleDateString(lang==='zh'?'zh-CN':'en-US',{year:date.getFullYear()===today.getFullYear()?undefined:'numeric',month:'2-digit',day:'2-digit'})+` ${time}`;
}
