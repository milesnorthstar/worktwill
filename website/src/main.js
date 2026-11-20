import {
  createIcons, Activity, ArrowDown, ArrowDownRight, ArrowRight, ArrowUpRight,
  Check, CircleCheck, Coffee, Compass, CornerDownLeft, Download, FileCheck2,
  Files, Folder, FolderOpen, FolderPlus, Globe, HardDrive, Info, Layers2,
  LayoutDashboard, Link, Monitor, MousePointer2, PanelsTopLeft, Plus, Search,
  ShieldCheck, Sprout, Star, UserRoundCheck,
} from 'lucide';
import release from '../release.json';
import demo from './demo.generated.js';

const icons = { Activity, ArrowDown, ArrowDownRight, ArrowRight, ArrowUpRight,
  Check, CircleCheck, Coffee, Compass, CornerDownLeft, Download, FileCheck2,
  Files, Folder, FolderOpen, FolderPlus, Globe, HardDrive, Info, Layers2,
  LayoutDashboard, Link, Monitor, MousePointer2, PanelsTopLeft, Plus, Search,
  ShieldCheck, Sprout, Star, UserRoundCheck };
createIcons({ icons, nameAttr: 'data-icon', attrs: { 'stroke-width': 1.65, 'aria-hidden': 'true' } });

const download = document.querySelector('#release-download');
if (download) download.href = `/downloads/${release.filename}`;
const checksum = document.querySelector('#release-sha');
if (checksum) checksum.textContent = release.sha256;

// 官网示例与桌面业务数据完全隔离，只演示检索和资料关联。
const { resources, workspaceNames, views, labels } = demo[document.documentElement.lang];
let view = 'home';
let space = null;
let filter = 'all';
const search = document.querySelector('#demo-search');
const list = document.querySelector('#demo-resources');

function render() {
  if (!list || !search) return;
  const query = search.value.trim().toLocaleLowerCase();
  const searching = Boolean(query);
  const title = space ? workspaceNames[space] : views[view][0];
  document.querySelector('#demo-title').textContent = searching ? labels.searchTitle : title;
  document.querySelector('#demo-description').textContent = searching ? labels.searchDescription : space ? labels.workspaceDescription : views[view][1];
  document.querySelector('#demo-breadcrumb').textContent = space ? `${labels.workspace} / ${workspaceNames[space]}` : views[view][2];
  document.querySelector('#demo-workspace-section').hidden = searching || space !== null || !['home', 'workspaces'].includes(view);
  document.querySelector('.demo-resource-section').hidden = !searching && ['workspaces', 'activity'].includes(view) && !space;
  document.querySelector('#demo-activity').hidden = view !== 'activity' || searching || space !== null;
  const matches = resources.filter((r) => (searching || !space || r.spaces.includes(space)) && (searching || view !== 'favorites' || r.favorite) && (filter === 'all' || r.kind === filter) && (!query || `${r.name} ${r.location}`.toLocaleLowerCase().includes(query)));
  document.querySelector('#demo-list-title').textContent = searching ? labels.results.replace('{count}', matches.length) : space ? labels.workspaceResources : view === 'favorites' ? labels.favorites : labels.recent;
  list.replaceChildren();
  for (const resource of matches.slice(0, 4)) {
    const row = document.createElement('div');
    row.className = 'resource-row';
    const name = document.createElement('span');
    name.className = 'resource-name';
    const icon = document.createElement('span');
    icon.className = `file-icon ${resource.icon}`;
    icon.textContent = resource.letter;
    const label = document.createElement('span');
    label.textContent = resource.name;
    name.append(icon, label);
    const location = document.createElement('span');
    location.className = 'resource-location';
    location.textContent = resource.location;
    const type = document.createElement('span');
    type.className = 'resource-type';
    type.textContent = resource.type;
    row.append(name, location, type);
    list.append(row);
  }
  if (!matches.length) {
    const empty = document.createElement('p');
    empty.className = 'demo-empty';
    empty.textContent = labels.empty;
    list.append(empty);
  }
  document.querySelectorAll('[data-view]').forEach((button) => {
    button.classList.toggle('selected', !space && button.dataset.view === view);
    button.setAttribute('aria-pressed', String(!space && button.dataset.view === view));
  });
  document.querySelectorAll('.demo-spaces [data-space]').forEach((button) => {
    button.classList.toggle('selected', button.dataset.space === space);
    button.setAttribute('aria-pressed', String(button.dataset.space === space));
  });
}

document.querySelectorAll('[data-view]').forEach((button) => button.addEventListener('click', () => {
  view = button.dataset.view;
  space = null;
  search.value = '';
  render();
}));
document.querySelectorAll('[data-space]').forEach((button) => button.addEventListener('click', () => {
  space = button.dataset.space;
  search.value = '';
  render();
}));
document.querySelectorAll('[data-filter]').forEach((button) => button.addEventListener('click', () => {
  filter = button.dataset.filter;
  document.querySelectorAll('[data-filter]').forEach((item) => {
    item.classList.toggle('selected', item === button);
    item.setAttribute('aria-pressed', String(item === button));
  });
  render();
}));
search?.addEventListener('input', render);
// 只在演示区域内接管快捷键，避免影响页面其他区域。
document.querySelector('.app-window')?.addEventListener('keydown', (event) => {
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
    event.preventDefault();
    search.focus();
  }
  if (event.key === 'Escape' && event.target === search) {
    search.value = '';
    render();
  }
});
render();
