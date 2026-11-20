
"use strict";

const icons = {
  brand: '<rect x="3" y="6" width="14" height="14" rx="3"/><path d="M7 6V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2h-2"/><path d="M7 11h6m-6 4h4"/>',
  home: '<path d="m3 10 9-7 9 7v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><path d="M9 21v-7h6v7"/>',
  layers: '<path d="m12 2 10 6-10 6L2 8zM2 12l10 6 10-6M2 16l10 6 10-6"/>',
  clock: '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>',
  folder: '<path d="M3 19h18V7H11L9 4H3z"/>',
  search: '<circle cx="11" cy="11" r="6.5"/><path d="m16 16 4 4"/>',
  plus: '<path d="M12 5v14M5 12h14"/>',
  arrow: '<path d="M4 12h15m-6-6 6 6-6 6"/>',
  chevron: '<path d="m9 6 6 6-6 6"/>',
  file: '<path d="M5 3h9l5 5v13H5zM14 3v6h5M8 13h8m-8 4h6"/>',
  grid: '<path d="M4 3h11l5 5v13H4zM15 3v6h5"/><path d="M7 12h10v6H7zm0 3h10m-6-3v6"/>',
  image: '<rect x="3" y="4" width="18" height="16" rx="2"/><circle cx="8" cy="9" r="1.4"/><path d="m4 17 5-4 3 2 4-5 5 5"/>',
  link: '<path d="M10 13a4 4 0 0 0 6 .5l4-4a4 4 0 0 0-5.7-5.6L12 6m2 5a4 4 0 0 0-6-.5l-4 4A4 4 0 0 0 9.7 20l2.3-2"/>',
  star: '<path d="m12 3 2.6 5.4 5.9.9-4.2 4.1 1 5.9-5.3-2.8-5.3 2.8 1-5.9-4.2-4.1 5.9-.9z"/>',
  external: '<path d="M13 4h7v7m0-7-9 9"/><path d="M10 5H5v15h15v-5"/>',
  settings: '<circle cx="12" cy="12" r="3"/><path d="m10 3-1 3-3 1-3 3v4l3 3 3 1 1 3h4l1-3 3-1 3-3v-4l-3-3-3-1-1-3z"/>',
  shield: '<path d="M12 22s7-3 7-10V5l-7-3-7 3v7c0 7 7 10 7 10z"/><path d="m9 12 2 2 4-4"/>',
  check: '<path d="m5 12 4 4L19 6"/>',
  close: '<path d="m6 6 12 12M18 6 6 18"/>',
  edit: '<path d="m4 19 4-.6 11-11-3.4-3.4-11 11L4 19zM14 6l4 4"/>',
  minus: '<path d="M5 12h14"/>',
  list: '<path d="M9 5h11M9 12h11M9 19h11M4 5h.1M4 12h.1M4 19h.1"/>',
  refresh: '<path d="M20 7v5h-5M4 17v-5h5M6 7a7 7 0 0 1 12-1l2 6M4 12l2 6a7 7 0 0 0 12-1"/>'
};

function icon(name, cssClass = "") {
  return `<svg class="${cssClass}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${icons[name]}</svg>`;
}

function escapeHtml(value) {
  return String(value).replace(/[&<>"']/g, (character) => ({
    "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;"
  })[character]);
}

const $ = (selector, root = document) => root.querySelector(selector);
const storageKey = "local-workspace-prototype-v1";
const now = Date.now();
const yesterday = now - 86400000;
const workspaceColors = ["purple", "green", "peach", "blue", "amber"];
const typeLabels = { workspace: "工作区", file: "文件", web: "网页" };
const entryTabs = [
  ["modified", "最近修改"], ["recent", "最近使用"], ["favorites", "收藏"], ["files", "全部文件"]
];

function demoFile(id, name, rootId, directory, time, size, missing = false) {
  return { id, type: "file", name, rootId, path: `${directory}/${name}`, time, size, missing };
}

const demo = {
  locations: [
    { id: "desktop", name: "桌面", active: true, demo: true },
    { id: "documents", name: "文档", active: true, demo: true },
    { id: "downloads", name: "下载", active: true, demo: true }
  ],
  resources: [
    demoFile("schedule", "发布排期表.xlsx", "documents", "文档/内容运营", now - 47 * 60000, 42432),
    demoFile("plan", "秋季发布方案.docx", "desktop", "桌面/品牌活动", now - 82 * 60000, 32876),
    demoFile("cover", "秋季发布_封面.png", "downloads", "下载/活动素材", now - 156 * 60000, 1865372),
    demoFile("review", "渠道投放复盘.pdf", "documents", "文档/数据复盘", yesterday, 605422),
    demoFile("contract", "合作合同_张三.docx", "documents", "文档/客户资料", yesterday - 65000, 73124),
    demoFile("archive", "渠道素材归档.zip", "downloads", "下载/活动素材", yesterday - 4680000, 1532489, true),
    { id: "board", type: "web", name: "发布后台", url: "https://www.notion.so/", time: yesterday },
    { id: "figma", type: "web", name: "设计稿 · Figma", url: "https://www.figma.com/", time: yesterday },
    { id: "metrics", type: "web", name: "渠道数据 · Analytics", url: "https://analytics.google.com/", time: yesterday }
  ],
  workspaces: [
    { id: "launch", name: "秋季内容发布", color: "purple", resources: ["schedule", "plan", "cover", "board", "figma"] },
    { id: "channel", name: "渠道数据复盘", color: "green", resources: ["schedule", "review", "archive", "metrics"] },
    { id: "delivery", name: "客户交付", color: "peach", resources: ["contract"] }
  ],
  favorites: ["schedule", "board"],
  lastWorkspace: null,
  events: []
};

const demoRepairPath = "下载/活动素材/渠道素材归档_已恢复.zip";

function readState() {
  try {
    const saved = localStorage.getItem(storageKey);
    if (saved) return JSON.parse(saved);
  } catch {
    // 浏览器阻止本地存储时，仍可在当前会话体验。
  }
  return { locations: [], resources: [], workspaces: [], favorites: [], lastWorkspace: null, events: [] };
}

let state = readState();
let persistenceFailed = false;

function saveState() {
  try {
    localStorage.setItem(storageKey, JSON.stringify(state));
  } catch {
    if (!persistenceFailed) notify("浏览器存储不可用，改动仅保留在本次会话");
    persistenceFailed = true;
  }
}

const ui = {
  view: "home",
  workspace: null,
  homeTab: "modified",
  filter: "all",
  sort: "relevance",
  historyDate: "all",
  historyType: "all",
  historyWorkspace: "all",
  selected: new Set(),
  searchOpen: false,
  scope: "all",
  searchIntent: "browse",
  results: null,
  resumeSearch: false,
  resultIndex: -1,
  quickIds: []
};

const search = $("#search");
const searchPanel = $("#search-panel");
const dialog = $("#dialog");
const shortcut = /Mac|iPhone|iPad/i.test(navigator.platform) ? "⌘ K" : "Ctrl + K";

function resourceById(id) {
  return state.resources.find((resource) => resource.id === id);
}

function workspaceById(id) {
  return state.workspaces.find((workspace) => workspace.id === id);
}

function currentWorkspace() {
  return workspaceById(ui.workspace);
}

function hasActiveLocation() {
  return state.locations.some((location) => location.active);
}

function hasDemoData() {
  return state.locations.some((location) => location.demo);
}

function isDemoWorkspace(workspace) {
  return demo.workspaces.some((example) => example.id === workspace.id);
}

function authorized(resource) {
  return resource.type === "web" || state.locations.some((location) => location.id === resource.rootId && location.active);
}

function showableName(resource) {
  return authorized(resource) ? resource.name : "已撤权的文件";
}

function extension(resource) {
  return resource.name.split(".").pop()?.toLowerCase();
}

function tile(item, small = false) {
  const ext = item.type === "file" ? extension(item) : "";
  const token = item.type === "workspace" ? item.color : item.type === "web" ? "web" : ext;
  const typeIcon = ["xlsx", "csv"].includes(ext) ? "grid" : ["jpg", "png", "webp", "jpeg"].includes(ext) ? "image" : "file";
  const symbol = item.type === "workspace" ? "folder" : item.type === "web" ? "link" : typeIcon;
  return `<span class="tile ${small ? "small" : ""} ${escapeHtml(token || "")}">${icon(symbol)}</span>`;
}

function button(label, action, symbol, cssClass = "", id = "") {
  return `<button type="button" class="button ${cssClass}" data-action="${action}" data-id="${escapeHtml(id)}">${symbol ? icon(symbol) : ""}${escapeHtml(label)}</button>`;
}

function iconButton(label, action, symbol, id = "", active = false) {
  return `<button type="button" class="icon-button${active ? " on" : ""}" data-action="${action}" data-id="${escapeHtml(id)}" title="${escapeHtml(label)}" aria-label="${escapeHtml(label)}">${icon(symbol)}</button>`;
}

function pageTitle(title, note = "", actions = "") {
  return `<div class="page-head"><div><h1>${escapeHtml(title)}</h1>${note ? `<p>${escapeHtml(note)}</p>` : ""}</div><div class="actions">${actions}</div></div>`;
}

function emptyState(message, symbol = "folder") {
  return `<div class="empty">${icon(symbol)}<p>${escapeHtml(message)}</p></div>`;
}

function selectControl(name, options, current, label) {
  return `<select class="select" data-select="${name}" aria-label="${escapeHtml(label)}">${options.map(([value, text]) => `<option value="${escapeHtml(value)}"${current === value ? " selected" : ""}>${escapeHtml(text)}</option>`).join("")}</select>`;
}

function displayTime(timestamp, withTime = false) {
  if (!timestamp) return "—";
  const date = new Date(timestamp);
  const today = new Date();
  const nextDay = new Date(date);
  nextDay.setDate(date.getDate() + 1);
  const hour = date.toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit", hour12: false });
  if (date.toDateString() === today.toDateString()) return `今天${withTime ? ` ${hour}` : ""}`;
  if (nextDay.toDateString() === today.toDateString()) return `昨天${withTime ? ` ${hour}` : ""}`;
  return date.toLocaleDateString("zh-CN", { month: "numeric", day: "numeric" });
}

function shortPath(resource) {
  if (!authorized(resource)) return "重新授权后可访问";
  if (resource.type === "web") return resource.url.replace(/^https?:\/\//i, "");
  if (resource.missing) return "原件不可用";
  return resource.path.slice(0, Math.max(0, resource.path.lastIndexOf("/")));
}

function sizeLabel(size) {
  if (!Number.isFinite(size)) return "—";
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${Math.round(size / 1024)} KB`;
  return `${(size / (1024 * 1024)).toFixed(1)} MB`;
}

let toastTimer;
function notify(message) {
  const toast = $("#toast");
  toast.innerHTML = `${icon("check")}${escapeHtml(message)}`;
  toast.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => { toast.hidden = true; }, 3000);
}

function activity(kind, resourceId = null, workspaceId = null) {
  state.events.unshift({ kind, resourceId, workspaceId, time: Date.now() });
}

function renderSidebar() {
  const nav = [
    ["home", "工作台", "home"],
    ["workspaces", "工作区", "layers"],
    ["activity", "动态", "clock"]
  ];
  $("#sidebar").innerHTML = `
    <button class="brand" data-action="navigate" data-id="home" title="工作台" aria-label="WorkTwill 工作台">
      <span class="brand-mark">${icon("brand")}</span><span class="brand-copy"><span class="brand-name">WorkTwill</span><span class="brand-caption">个人工作台</span></span>
    </button>
    <nav class="menu">${nav.map(([view, label, symbol]) => {
      const active = ui.view === view || (ui.view === "workspace" && view === "workspaces");
      return `<button class="nav${active ? " active" : ""}" data-action="navigate" data-id="${view}" title="${label}" aria-label="${label}" ${active ? 'aria-current="page"' : ""}>${icon(symbol)}<span class="nav-text">${label}</span></button>`;
    }).join("")}</nav>
    <div class="side-section">
      <div class="side-label">我的工作区 ${iconButton("新建工作区", "new-workspace", "plus")}</div>
      <div class="menu">${state.workspaces.map((workspace) =>
        `<button class="nav${ui.workspace === workspace.id && ui.view === "workspace" ? " active" : ""} ${workspace.color}" data-action="open-workspace" data-id="${workspace.id}" data-drop-workspace="${workspace.id}" title="${escapeHtml(workspace.name)}" aria-label="${escapeHtml(workspace.name)}"><span class="dot"></span><span class="nav-text">${escapeHtml(workspace.name)}</span></button>`
      ).join("")}</div>
    </div>
    <div class="side-foot">
      <button class="nav" data-action="locations" title="管理位置" aria-label="管理位置">${icon("settings")}<span class="nav-text">管理位置</span></button>
      <div class="side-status" title="本地交互原型">${icon("shield")}<span>本地交互原型</span></div>
    </div>`;
}

function workspaceCard(workspace) {
  const resources = workspace.resources.map(resourceById).filter(Boolean);
  const fileCount = resources.filter((resource) => resource.type === "file").length;
  const webCount = resources.length - fileCount;
  return `<button class="workspace-card" data-action="open-workspace" data-id="${workspace.id}" data-drop-workspace="${workspace.id}">
    ${tile({ ...workspace, type: "workspace" })}
    <span class="workspace-card-copy"><strong>${escapeHtml(workspace.name)}</strong><small>${isDemoWorkspace(workspace) ? "示例 · " : ""}${fileCount} 个文件 · ${webCount} 个网页</small></span>
    ${icon("chevron", "card-arrow")}
  </button>`;
}

function sourceList(controls = false) {
  if (!state.locations.length) return emptyState("还没有管理位置");
  return state.locations.map((location) => {
    const count = state.resources.filter((resource) => resource.rootId === location.id).length;
    const meta = location.active ? `${count} 个文件${location.demo ? " · 示例目录" : ""}` : "权限已撤销";
    return `<div class="source-row">
      <span class="source-icon">${icon("folder")}</span>
      <div class="source-name">${escapeHtml(location.name)}<div class="source-meta">${meta}</div></div>
      ${controls ? button(location.active ? "撤销" : "重新授权", "toggle-location", "", "", location.id) : `<span class="source-state${location.active ? "" : " off"}">${location.active ? "已索引" : "已撤权"}</span>`}
    </div>`;
  }).join("");
}

function resourceTable(resources, options = {}) {
  if (!resources.length) return emptyState(options.emptyText || "这里还没有资源");
  const checkedCount = resources.filter((resource) => ui.selected.has(resource.id)).length;
  const selectableCount = resources.filter(authorized).length;
  const allChecked = selectableCount > 0 && checkedCount === selectableCount;
  const selectionBar = checkedCount ? `<div class="selection-bar"><strong>已选 ${checkedCount} 项</strong>
    ${button("加入工作区", "add-selected", "plus", "primary")}
    ${options.workspace ? button("从当前工作区移除", "remove-selected", "minus") : ""}
    ${button("清空选择", "clear-selection", "", "ghost")}</div>` : "";
  return `${selectionBar}<div class="table-scroll"><table class="resource-table">
    <thead><tr><th scope="col" class="check"><input type="checkbox" data-check="all" aria-label="全选可访问资源"${allChecked ? " checked" : ""}${selectableCount ? "" : " disabled"}></th><th scope="col">名称</th><th scope="col" class="date">${options.timeLabel || "修改时间"}</th><th scope="col" class="row-actions">操作</th></tr></thead>
    <tbody>${resources.map((resource) => {
      const available = authorized(resource);
      const id = resource.id;
      const favorited = state.favorites.includes(id);
      const name = showableName(resource);
      return `<tr data-resource="${id}"${available ? ' draggable="true"' : ""}${ui.selected.has(id) ? ' class="selected"' : ""}>
        <td class="check"><input type="checkbox" data-check="${id}" aria-label="选择 ${escapeHtml(name)}"${ui.selected.has(id) ? " checked" : ""}${available ? "" : " disabled"}></td>
        <td><button class="resource-name" data-action="details" data-id="${id}" title="${escapeHtml(name)}">${tile(available ? resource : { type: "file", name: "" }, true)}<span class="row-title"><strong>${escapeHtml(name)}</strong><small>${escapeHtml(shortPath(resource))}</small></span></button></td>
        <td class="date">${available ? displayTime(options.times?.get(id) ?? resource.time, true) : "—"}</td>
        <td class="row-actions">${available ? iconButton("打开", "open-resource", "external", id) + iconButton(favorited ? "取消收藏" : "收藏", "favorite", "star", id, favorited) : ""}
        ${options.workspace ? iconButton("从当前工作区移除", "request-unlink", "minus", id) : available ? iconButton("加入工作区", "add-resource", "plus", id) : ""}</td>
      </tr>`;
    }).join("")}</tbody></table></div>`;
}

function renderHome() {
  if (!state.locations.length && !state.resources.length && !state.workspaces.length) {
    return `${pageTitle("工作台", "", button("添加网页", "new-web", "link"))}
      <section class="hero">
        <div class="hero-copy"><h2>从你常用的文件夹开始</h2><p>选择目录后即可搜索文件，文件保存在原来的位置。</p>
        <div class="actions">${button("添加管理位置", "choose-directory", "folder", "primary")}${button("体验示例", "load-demo", "layers")}</div></div>
        ${heroArt}
      </section>`;
  }
  const actions = button("添加网页", "new-web", "link") +
    button("新建工作区", "new-workspace", "plus", hasActiveLocation() ? "primary" : "") +
    (hasActiveLocation() ? "" : button("添加管理位置", "choose-directory", "folder", "primary"));
  const last = workspaceById(state.lastWorkspace);
  const sample = state.workspaces.find(isDemoWorkspace);
  const featured = last || sample;
  const opens = new Map();
  for (const event of state.events) {
    if (event.kind === "opened" && resourceById(event.resourceId)?.type === "web" && !opens.has(event.resourceId)) opens.set(event.resourceId, event.time);
  }
  const resources = state.resources.filter(authorized);
  const entries = {
    modified: resources.filter((resource) => resource.type === "file").sort((a, b) => b.time - a.time).slice(0, 5),
    recent: [...opens.keys()].map(resourceById).filter((resource) => resource && authorized(resource)),
    favorites: resources.filter((resource) => state.favorites.includes(resource.id)),
    files: resources.filter((resource) => resource.type === "file").sort((a, b) => a.name.localeCompare(b.name, "zh-CN"))
  };
  const emptyText = { modified: "添加管理位置后可查看最近修改的文件", recent: "从工作台打开资源后会显示在这里", favorites: "点击资源旁的星标即可收藏", files: "添加管理位置后可在这里查看文件" };
  const hero = featured ? `<section class="hero">
    <div class="hero-copy"><div class="hero-label">${icon("clock")}${isDemoWorkspace(featured) ? "示例工作区" : "继续上次的工作"}</div>
    <h2>${escapeHtml(featured.name)}</h2><p>${featured.resources.length} 个资源</p>${button("打开工作区", "open-workspace", "arrow", "primary", featured.id)}</div>
    ${heroArt}
  </section>` : "";
  return `${pageTitle("工作台", "找回最近用过的资料，继续手头的工作。", actions)}
    ${hero}
    <section class="block">
      <div class="block-heading"><h2>我的工作区</h2><button class="text-button" data-action="navigate" data-id="workspaces">查看全部 ${icon("chevron")}</button></div>
      ${state.workspaces.length ? `<div class="cards">${state.workspaces.slice(0, 3).map(workspaceCard).join("")}</div>` : `<div class="pane">${emptyState("创建工作区，把同一件工作的文件和网页放在一起")}</div>`}
    </section>
    <div class="block lower-grid"><section class="pane">
      <div class="pane-head"><h2 class="pane-title">资源</h2><div class="tabs" aria-label="资源分类">${entryTabs.map(([id, label]) => `<button class="tab${id === ui.homeTab ? " active" : ""}" aria-pressed="${id === ui.homeTab}" data-action="home-tab" data-id="${id}">${label}</button>`).join("")}</div></div>
      ${resourceTable(entries[ui.homeTab], { times: ui.homeTab === "recent" ? opens : null, timeLabel: ui.homeTab === "recent" ? "打开时间" : "修改时间", emptyText: emptyText[ui.homeTab] })}
    </section>
    <aside class="pane">
      <div class="pane-head"><h2 class="pane-title">管理位置</h2><button class="text-button" data-action="locations">管理 ${icon("chevron")}</button></div>
      ${sourceList()}<div class="pane-note">搜索仅覆盖已授权位置和已保存的网页、工作区。</div>
    </aside></div>`;
}

const heroArt = `<div class="hero-art" aria-hidden="true"><div class="hero-orbit"></div><div class="art-paper one">${icon("file")}</div><div class="art-folder">${icon("layers")}</div><div class="art-paper two">${icon("link")}</div><div class="art-paper three">${icon("grid")}</div></div>`;

function renderWorkspaces() {
  const actions = button("新建工作区", "new-workspace", "plus", "primary");
  const content = state.workspaces.length ? `<div class="cards">${state.workspaces.map(workspaceCard).join("")}</div>` : `<div class="pane">${emptyState("还没有工作区")}</div>`;
  return pageTitle("工作区", "", actions) + content;
}

function filterTabs() {
  if (ui.view === "results" && ui.results.intent === "add-files") return "<strong class=\"pane-title\">文件</strong>";
  const options = ui.view === "results" ? [["all", "全部"], ["workspace", "工作区"], ["file", "文件"], ["web", "网页"]] : [["all", "全部"], ["file", "文件"], ["web", "网页"]];
  return `<div class="tabs" aria-label="按资源类型筛选">${options.map(([id, label]) => `<button class="tab${ui.filter === id ? " active" : ""}" aria-pressed="${ui.filter === id}" data-action="filter" data-id="${id}">${label}</button>`).join("")}</div>`;
}

function renderWorkspace() {
  const workspace = workspaceById(ui.workspace);
  const actions = button("重命名", "rename-workspace", "edit", "", workspace.id) +
    button("删除工作区", "delete-workspace", "", "", workspace.id) +
    button("添加网页", "new-web", "link", "", workspace.id) +
    button("添加文件", "search-files-to-add", "plus", "primary");
  const resources = workspace.resources.map(resourceById).filter((resource) => resource && (ui.filter === "all" || ui.filter === resource.type));
  const note = `${isDemoWorkspace(workspace) ? "示例工作区 · " : ""}${workspace.resources.length} 个资源 · 文件保存在原来的位置`;
  return `${pageTitle(workspace.name, note, actions)}
    <section class="pane" data-drop-workspace="${workspace.id}">
      <div class="pane-head"><h2 class="pane-title">工作区资源</h2>${filterTabs()}</div>
      ${resourceTable(resources, { workspace: true, emptyText: workspace.resources.length ? "当前类型没有资源" : "搜索文件并加入工作区，或添加网页" })}
    </section>`;
}

function renderActivity() {
  const events = [...state.events];
  for (const resource of state.resources) {
    if (resource.type === "file" && authorized(resource) && !resource.missing) {
      events.push({ kind: "modified", resourceId: resource.id, time: resource.time });
    }
  }
  events.sort((a, b) => b.time - a.time);
  const visible = events.filter((event) => {
    const resource = resourceById(event.resourceId);
    if (resource && !authorized(resource)) return false;
    if (ui.historyDate !== "all" && displayTime(event.time) !== ui.historyDate) return false;
    if (ui.historyType !== "all" && resource?.type !== ui.historyType) return false;
    if (ui.historyWorkspace === "all") return true;
    if (event.workspaceId) return event.workspaceId === ui.historyWorkspace;
    return workspaceById(ui.historyWorkspace)?.resources.includes(event.resourceId);
  });
  const types = [["all", "全部资源类型"], ["file", "文件"], ["web", "网页"]];
  const workspaces = [["all", "全部工作区"], ...state.workspaces.map((workspace) => [workspace.id, workspace.name])];
  const filters = selectControl("historyDate", [["all", "全部日期"], ["今天", "今天"], ["昨天", "昨天"]], ui.historyDate, "日期") +
    selectControl("historyType", types, ui.historyType, "资源类型") +
    selectControl("historyWorkspace", workspaces, ui.historyWorkspace, "工作区");
  const labels = { modified: "文件修改时间", opened: "应用内打开", simulated_open: "模拟打开文件", linked: "已关联", unlinked: "已移除关联", created: "已创建工作区", repaired: "已重新指向原件" };
  const rows = visible.map((event) => {
    const resource = resourceById(event.resourceId);
    const workspace = workspaceById(event.workspaceId);
    const title = resource?.name || workspace?.name || "工作区";
    const action = resource ? "details" : "open-workspace";
    const id = resource?.id || workspace?.id;
    return `<li>${resource ? tile(resource, true) : `<span class="tile small">${icon("layers")}</span>`}
      <span class="row-title">${id ? `<button class="text-button" data-action="${action}" data-id="${id}">${escapeHtml(title)}</button>` : `<strong>${escapeHtml(title)}</strong>`}<small>${labels[event.kind === "opened" && resource?.type === "file" ? "simulated_open" : event.kind]}${resource && workspace ? ` · ${escapeHtml(workspace.name)}` : ""}</small></span>
      <time>${displayTime(event.time, true)}</time></li>`;
  }).join("");
  return `${pageTitle("动态", "已索引文件的最近修改与本应用内的操作。")}
    <section class="pane"><div class="pane-head"><h2 class="pane-title">活动记录</h2><div class="actions">${filters}</div></div>
    ${rows ? `<ul class="timeline">${rows}</ul>` : emptyState("没有符合条件的动态", "clock")}</section>`;
}

function renderResults() {
  const results = findResults(ui.results.query, ui.results.scope, ui.results.intent);
  const shown = results.filter((item) => ui.filter === "all" || item.type === ui.filter);
  if (ui.sort === "name") shown.sort((a, b) => a.name.localeCompare(b.name, "zh-CN"));
  if (ui.sort === "time") shown.sort((a, b) => (b.time || 0) - (a.time || 0));
  const workspaces = shown.filter((item) => item.type === "workspace");
  const resources = shown.filter((item) => item.type !== "workspace");
  const useCards = ui.filter === "workspace" || workspaces.length;
  const workspace = ui.results.origin === "workspace" ? currentWorkspace() : null;
  const back = workspace ? button(`返回「${workspace.name}」`, "return-to-workspace", "chevron") : "";
  const scopeLabel = ui.results.intent === "add-files" ? "全部已授权文件" : ui.results.scope === "all" ? "全部资源" : "当前工作区";
  return `${pageTitle(`搜索「${ui.results.query}」`, `${shown.length} 条结果 · ${scopeLabel}`, back)}
    <section class="pane"><div class="pane-head">${filterTabs()}<div class="actions">${selectControl("sort", [["relevance", "按相关性"], ["name", "按名称"], ["time", "按时间"]], ui.sort, "排序方式")}</div></div>
      ${resources.length || !useCards ? resourceTable(resources, { emptyText: "没有符合条件的结果" }) : ""}
    </section>${workspaces.length ? `<section class="block"><div class="block-heading"><h2>工作区</h2></div><div class="cards">${workspaces.map(workspaceCard).join("")}</div></section>` : ""}`;
}

function renderPage() {
  const labels = { home: "工作台", workspaces: "工作区", workspace: "工作区", activity: "动态", results: "搜索结果" };
  const workspace = ui.view === "workspace" && currentWorkspace();
  $("#breadcrumb").innerHTML = workspace ? `<span>工作区 /</span>${escapeHtml(workspace.name)}` : escapeHtml(labels[ui.view]);
  $("#page").innerHTML = { home: renderHome, workspaces: renderWorkspaces, workspace: renderWorkspace, activity: renderActivity, results: renderResults }[ui.view]();
}

function render() {
  renderSidebar();
  renderPage();
  if (ui.searchOpen) renderSearch();
}

function navigate(view, workspaceId = null) {
  if (view === "workspace" && !workspaceById(workspaceId)) return;
  ui.view = view;
  if (view !== "results") {
    ui.workspace = workspaceId;
    ui.scope = "all";
    ui.results = null;
  }
  ui.resumeSearch = false;
  ui.searchIntent = "browse";
  ui.filter = "all";
  ui.selected.clear();
  if (workspaceId && !isDemoWorkspace(currentWorkspace())) {
    state.lastWorkspace = workspaceId;
    saveState();
  }
  dismissSearch();
  if (view !== "results") search.value = "";
  render();
  $("#page").focus({ preventScroll: true });
  window.scrollTo(0, 0);
}


function normalize(value) {
  return value.normalize("NFKC").toLocaleLowerCase();
}

function rank(item, query, terms) {
  const title = normalize(item.name);
  const destination = normalize(item.path || item.url || "");
  if (!terms.every((term) => title.includes(term) || destination.includes(term))) return 0;
  if (title === query) return 4;
  if (title.startsWith(query)) return 3;
  if (title.includes(query) || terms.every((term) => title.includes(term))) return 2;
  return 1;
}

function findResults(rawQuery, scope, intent) {
  const query = normalize(rawQuery.trim());
  if (!query) return [];
  const terms = query.split(/\s+/);
  const filesOnly = intent === "add-files";
  const workspace = !filesOnly && scope === "workspace" && currentWorkspace();
  const resources = state.resources.filter((resource) => authorized(resource) &&
    (!filesOnly || resource.type === "file") && (!workspace || workspace.resources.includes(resource.id)));
  const workspaces = filesOnly ? [] : (workspace ? [workspace] : state.workspaces).map((item) => ({ ...item, type: "workspace" }));
  return [...workspaces, ...resources]
    .map((item) => ({ item, score: rank(item, query, terms) }))
    .filter(({ score }) => score)
    .sort((a, b) => b.score - a.score || (b.item.time || 0) - (a.item.time || 0))
    .map(({ item }) => item);
}

function highlight(text, query) {
  const term = query.trim().split(/\s+/).find((part) => text.toLowerCase().includes(part.toLowerCase()));
  if (!term) return escapeHtml(text);
  const start = text.toLowerCase().indexOf(term.toLowerCase());
  const end = start + term.length;
  return `${escapeHtml(text.slice(0, start))}<mark>${escapeHtml(text.slice(start, end))}</mark>${escapeHtml(text.slice(end))}`;
}

function renderSearch() {
  const query = search.value;
  const filesOnly = ui.searchIntent === "add-files";
  const results = findResults(query, ui.scope, ui.searchIntent);
  const groups = ["workspace", "file", "web"].map((type) => [type, results.filter((item) => item.type === type).slice(0, 5)]);
  const previousId = ui.quickIds[ui.resultIndex]?.id;
  ui.quickIds = groups.flatMap(([, items]) => items);
  const normalizedQuery = normalize(query.trim());
  const ambiguousName = results.filter((item) => normalize(item.name) === normalizedQuery).length > 1;
  // 分组决定展示位置，默认选中项仍按全局相关性决定。同名时让用户先选路径。
  const selectedId = previousId || (ambiguousName ? null : results[0]?.id);
  ui.resultIndex = ui.quickIds.findIndex((item) => item.id === selectedId);
  const workspace = currentWorkspace();
  const scopePicker = filesOnly ? "<strong>全部已授权文件</strong>" : workspace
    ? selectControl("scope", [["all", "全部资源"], ["workspace", "当前工作区"]], ui.scope, "搜索范围") : "<strong>全部资源</strong>";
  const rows = groups.filter(([, items]) => items.length).map(([type, items]) => `
    <div role="group" aria-label="${typeLabels[type]}"><div class="result-group" aria-hidden="true">${typeLabels[type]}</div>
    ${items.map((item) => {
      const label = item.type === "workspace" ? `${item.resources.length} 个资源` : shortPath(item);
      const openAction = item.type === "workspace" ? "open-workspace" : "open-resource";
      return `<div class="result-line" role="presentation"><button type="button" role="option" aria-selected="false" id="quick-${item.id}" class="result-option" data-action="${openAction}" data-id="${item.id}">
        ${tile(item, true)}<span class="row-title"><strong>${highlight(item.name, query)}</strong><small>${highlight(label, query)}</small></span></button>
        ${item.type === "workspace" ? "" : iconButton("加入工作区", "add-resource", "plus", item.id)}</div>`;
    }).join("")}</div>`).join("");
  let message = query.trim() ? "未找到匹配的资源，试试更短的关键词" : filesOnly ? "输入文件名或路径" : "输入文件名、路径、网页标题或工作区名称";
  let emptyAction = "";
  if (!filesOnly && ui.scope === "workspace") {
    emptyAction = button("搜索全部资源", "search-all-resources", "search");
  } else if (!hasActiveLocation()) {
    message = "添加管理位置后即可搜索文件";
    emptyAction = button("添加管理位置", "choose-directory", "folder", "primary");
  }
  searchPanel.innerHTML = `<div class="search-panel-top"><span>${filesOnly && workspace ? `添加到「${escapeHtml(workspace.name)}」` : "搜索范围"}</span>${scopePicker}</div>
    <div id="quick-options" role="listbox" aria-label="快速搜索结果" class="quick-options">${rows || emptyState(message, "search")}</div>
    ${!rows && emptyAction ? `<div class="search-empty-actions">${emptyAction}</div>` : ""}
    <div class="search-footer"><span class="keys">${ambiguousName && ui.resultIndex === -1 ? "有多个同名结果，请按位置选择 · " : ""}<kbd>↑</kbd> <kbd>↓</kbd> 选择 · <kbd>Enter</kbd> 打开 · <kbd>Esc</kbd> 关闭</span>
    ${results.length ? `<button class="text-button" data-action="all-results">查看全部 ${results.length} 条 ${icon("arrow")}</button>` : ""}</div>`;
  updateActiveResult(true);
}

function updateActiveResult(scroll = false) {
  searchPanel.querySelectorAll(".result-option").forEach((option, index) => {
    const active = index === ui.resultIndex;
    option.setAttribute("aria-selected", String(active));
    option.parentElement.classList.toggle("active", active);
  });
  const active = ui.quickIds[ui.resultIndex];
  if (active) {
    search.setAttribute("aria-activedescendant", `quick-${active.id}`);
    if (scroll) {
      const list = $("#quick-options");
      const row = document.getElementById(`quick-${active.id}`).getBoundingClientRect();
      const bounds = list.getBoundingClientRect();
      if (row.top < bounds.top) list.scrollTop -= bounds.top - row.top;
      if (row.bottom > bounds.bottom) list.scrollTop += row.bottom - bounds.bottom;
    }
  } else {
    search.removeAttribute("aria-activedescendant");
  }
}

function openSearch() {
  if (ui.searchOpen) return;
  ui.searchOpen = true;
  ui.resultIndex = -1;
  searchPanel.hidden = false;
  search.setAttribute("aria-expanded", "true");
  renderSearch();
}

function dismissSearch() {
  ui.searchOpen = false;
  ui.resultIndex = -1;
  searchPanel.hidden = true;
  search.setAttribute("aria-expanded", "false");
  search.removeAttribute("aria-activedescendant");
}

function focusSearch(intent = "browse") {
  if (intent !== ui.searchIntent) {
    ui.searchIntent = intent;
    ui.resultIndex = -1;
    if (ui.searchOpen) renderSearch();
  }
  search.focus();
  search.select();
  openSearch();
}

function showDialog(title, body) {
  dismissSearch();
  dialog.innerHTML = `<div class="dialog-head"><h2 id="dialog-title">${escapeHtml(title)}</h2>${iconButton("关闭", "close-dialog", "close")}</div><div class="dialog-body">${body}</div>`;
  if (!dialog.open) dialog.showModal();
  dialog.querySelector("[data-autofocus]")?.focus();
}

function formDialog(kind, title, body, submit, extra = "", ids = []) {
  showDialog(title, `<form data-form="${kind}" data-id="${escapeHtml(ids[0] || "")}" data-ids="${escapeHtml(ids.join(","))}">
    ${body}<p class="form-error" aria-live="polite" hidden></p>
    <div class="dialog-actions">${extra}${button("取消", "close-dialog")}<button class="button primary" type="submit">${escapeHtml(submit)}</button></div>
  </form>`);
}

function field(name, label, value = "", placeholder = "", maxLength = 120) {
  return `<label class="form-field">${label}<input type="text" name="${name}" value="${escapeHtml(value)}" placeholder="${escapeHtml(placeholder)}" maxlength="${maxLength}" data-autofocus></label>`;
}

function setFormError(form, message) {
  const feedback = $(".form-error", form);
  feedback.textContent = message;
  feedback.hidden = false;
}

function makeId() {
  return crypto.randomUUID();
}

function createWorkspace(name) {
  const workspace = { id: makeId(), name, color: workspaceColors[state.workspaces.length % workspaceColors.length], resources: [] };
  state.workspaces.push(workspace);
  activity("created", null, workspace.id);
  return workspace;
}

function openWorkspace(id) {
  navigate("workspace", id);
}

function linkResources(ids, workspaces) {
  let count = 0;
  for (const workspace of workspaces) {
    for (const id of ids) {
      const resource = resourceById(id);
      if (!resource || !authorized(resource) || workspace.resources.includes(id)) continue;
      workspace.resources.push(id);
      activity("linked", id, workspace.id);
      count++;
    }
  }
  ui.selected.clear();
  saveState();
  render();
  notify(count ? (workspaces.length === 1 ? `已加入「${workspaces[0].name}」` : `已添加 ${count} 条工作区关联`) : "所选资源已在工作区中");
}

function addResourceDialog(ids) {
  if (!ids.length) return;
  const preferred = ui.workspace || state.lastWorkspace;
  const choices = state.workspaces.map((workspace) => {
    const linked = ids.every((id) => workspace.resources.includes(id));
    return `<label class="choice"><input type="checkbox" name="workspace" value="${workspace.id}"${linked ? " checked disabled" : workspace.id === preferred ? " checked" : ""}>${escapeHtml(workspace.name)}${linked ? " · 已加入" : ""}</label>`;
  }).join("");
  const label = ids.length === 1 ? showableName(resourceById(ids[0])) : `${ids.length} 个资源`;
  const extra = button("新建工作区", "new-workspace-for", "plus", "ghost", ids.join(","));
  formDialog("assign", "加入工作区", `<p>${escapeHtml(label)}</p>${choices || "<p>创建一个工作区后即可加入。</p>"}`, "加入所选工作区", extra, ids);
}

function requestUnlink(ids) {
  const workspace = currentWorkspace();
  if (!workspace || !ids.length) return;
  formDialog("unlink", "从当前工作区移除", `<p>将 ${ids.length} 个资源从「${escapeHtml(workspace.name)}」移除。其他工作区中的引用会保留。</p>`, "移除关联", "", ids);
}

function openDetails(resource) {
  if (!resource) return;
  if (!authorized(resource)) {
    showDialog("已撤权的文件", `<p>当前工作区保留了这条关联。重新授权后可查看原件。</p><div class="dialog-actions">${button("管理位置", "locations", "folder", "primary")}</div>`);
    return;
  }
  const pairs = resource.type === "web" ? [["网址", resource.url]] : [
    ["位置", resource.path], ["文件大小", sizeLabel(resource.size)],
    ["修改时间", displayTime(resource.time, true)], ["状态", resource.missing ? "原件不可用" : "已索引"]
  ];
  const detail = `<dl class="detail-list">${pairs.map(([label, value]) => `<dt>${label}</dt><dd>${escapeHtml(value)}</dd>`).join("")}</dl>`;
  const detailActions = resource.type === "web" ? button("编辑网页", "edit-web", "edit", "", resource.id)
    : resource.missing ? button("重新指向", "repair", "refresh", "", resource.id)
    : button("显示原始位置", "locate", "folder", "", resource.id);
  const mainAction = resource.missing ? "" : button("打开", "open-resource", "external", "primary", resource.id);
  showDialog(resource.name, detail + `<div class="dialog-actions">${detailActions}${button("加入工作区", "add-resource", "plus", "", resource.id)}${mainAction}</div>`);
}

function openResource(id) {
  const resource = resourceById(id);
  if (!resource) return;
  if (!authorized(resource) || resource.missing) {
    openDetails(resource);
    return;
  }
  dismissSearch();
  if (dialog.open) dialog.close();
  activity(resource.type === "file" ? "simulated_open" : "opened", id, ui.workspace);
  saveState();
  render();
  if (resource.type === "web") {
    window.open(resource.url, "_blank", "noopener,noreferrer");
    return;
  }
  showDialog("模拟打开文件", `<p>桌面版会用系统默认应用打开此文件。</p><dl class="detail-list"><dt>文件</dt><dd>${escapeHtml(resource.name)}</dd><dt>位置</dt><dd>${escapeHtml(resource.path)}</dd></dl><div class="dialog-actions">${button("完成", "close-dialog", "check", "primary")}</div>`);
}

function showLocations() {
  showDialog("管理位置", `<p>选择目录后，原型仅记录文件名、相对路径、大小和修改时间。</p>
    <div class="location-controls">${sourceList(true)}</div>
    <div class="dialog-actions">${hasDemoData() ? button("清空示例数据", "clear-demo", "", "ghost") : button("体验示例", "load-demo", "layers", "ghost")}${button("添加管理位置", "choose-directory", "plus", "primary")}</div>`);
}

function openWebForm(workspaceId, resourceId = null) {
  const resource = resourceId ? resourceById(resourceId) : null;
  formDialog(resource ? "edit-web" : "web", resource ? "编辑网页" : "添加网页", field("title", "网页标题", resource?.name || "", "例如 发布后台") +
    field("url", "网址", resource?.url || "", "https://", 2048), "保存网页", "", [resourceId || workspaceId].filter(Boolean));
}

function safeWebUrl(input) {
  try {
    const url = new URL(input.trim());
    if (["http:", "https:"].includes(url.protocol) && url.hostname) return url.href;
  } catch {
    // 非法网址由表单统一提示。
  }
  return "";
}

function finishDialog(message) {
  if (dialog.open) dialog.close();
  saveState();
  render();
  notify(message);
}

function loadDemo() {
  if (hasDemoData()) return;
  const example = structuredClone(demo);
  state.locations.push(...example.locations);
  state.resources.push(...example.resources);
  state.workspaces.push(...example.workspaces);
  state.favorites.push(...example.favorites);
  finishDialog("已加载示例数据");
}

function clearDemo() {
  const demoLocationIds = new Set(demo.locations.map((item) => item.id));
  const demoResourceIds = new Set(demo.resources.map((item) => item.id));
  const demoWorkspaceIds = new Set(demo.workspaces.map((item) => item.id));
  state.locations = state.locations.filter((item) => !demoLocationIds.has(item.id));
  state.resources = state.resources.filter((item) => !demoResourceIds.has(item.id));
  state.workspaces = state.workspaces.filter((item) => !demoWorkspaceIds.has(item.id));
  for (const workspace of state.workspaces) {
    workspace.resources = workspace.resources.filter((id) => !demoResourceIds.has(id));
  }
  state.favorites = state.favorites.filter((id) => !demoResourceIds.has(id));
  state.events = state.events.filter((event) => !demoResourceIds.has(event.resourceId) && !demoWorkspaceIds.has(event.workspaceId));
  if (demoWorkspaceIds.has(state.lastWorkspace)) state.lastWorkspace = null;
  if (demoWorkspaceIds.has(ui.workspace)) {
    navigate("home");
  }
  finishDialog("已清空示例数据");
}

function handleForm(event) {
  const form = event.target.closest("form[data-form]");
  if (!form) return;
  event.preventDefault();
  const values = new FormData(form);
  const id = form.dataset.id;
  const ids = form.dataset.ids.split(",").filter(Boolean);
  switch (form.dataset.form) {
    case "workspace":
    case "rename": {
      const name = values.get("name").trim();
      if (!name) return setFormError(form, "请输入工作区名称");
      if (form.dataset.form === "rename") {
        workspaceById(id).name = name;
        finishDialog("已重命名工作区");
      } else {
        const workspace = createWorkspace(name);
        dialog.close();
        if (ids.length) linkResources(ids, [workspace]);
        openWorkspace(workspace.id);
        if (!ids.length) notify("已创建工作区");
      }
      break;
    }
    case "delete": {
      state.workspaces = state.workspaces.filter((workspace) => workspace.id !== id);
      state.events = state.events.filter((item) => item.workspaceId !== id);
      if (state.lastWorkspace === id) state.lastWorkspace = state.workspaces[0]?.id || null;
      if (ui.historyWorkspace === id) ui.historyWorkspace = "all";
      dialog.close();
      saveState();
      navigate("workspaces");
      notify("已删除工作区");
      break;
    }
    case "web":
    case "edit-web": {
      const url = safeWebUrl(values.get("url"));
      if (!url) return setFormError(form, "请输入完整的 http 或 https 网址");
      const name = values.get("title").trim() || new URL(url).hostname;
      if (form.dataset.form === "edit-web") {
        Object.assign(resourceById(id), { name, url });
        finishDialog("已更新网页");
      } else {
        const resource = { id: makeId(), type: "web", url, name, time: Date.now() };
        state.resources.push(resource);
        const workspace = workspaceById(id);
        if (workspace) {
          workspace.resources.push(resource.id);
          activity("linked", resource.id, workspace.id);
          ui.filter = "all";
        }
        finishDialog(workspace ? `网页已加入「${workspace.name}」` : "网页已保存，可从全局搜索找回");
      }
      break;
    }
    case "assign": {
      const workspaces = values.getAll("workspace").map(workspaceById).filter(Boolean);
      if (!workspaces.length) return setFormError(form, "请选择要加入的工作区");
      dialog.close();
      linkResources(ids, workspaces);
      break;
    }
    case "unlink": {
      const workspace = currentWorkspace();
      workspace.resources = workspace.resources.filter((resourceId) => !ids.includes(resourceId));
      for (const resourceId of ids) activity("unlinked", resourceId, workspace.id);
      ui.selected.clear();
      finishDialog("已移除工作区关联");
      break;
    }
    case "repair": {
      const resource = resourceById(id);
      if (!authorized(resource)) return setFormError(form, "请先重新授权管理位置");
      if (id !== "archive" || values.get("replacement") !== demoRepairPath) return setFormError(form, "请选择可用的示例文件");
      Object.assign(resource, { path: demoRepairPath, name: demoRepairPath.split("/").at(-1), missing: false });
      activity("repaired", id, ui.workspace);
      finishDialog("已更新示例文件位置");
      break;
    }
    case "clear-demo":
      clearDemo();
      break;
  }
}

function handleAction(action, id, control) {
  switch (action) {
    case "navigate":
      navigate(id);
      break;
    case "open-workspace":
      openWorkspace(id);
      break;
    case "search-files-to-add":
      focusSearch("add-files");
      break;
    case "search-all-resources":
      ui.scope = "all";
      ui.resultIndex = -1;
      renderSearch();
      search.focus({ preventScroll: true });
      break;
    case "all-results":
      ui.results = {
        query: search.value.trim(),
        scope: ui.scope,
        intent: ui.searchIntent,
        origin: ui.view === "results" ? ui.results.origin : ui.view,
        originFilter: ui.view === "results" ? ui.results.originFilter : ui.filter
      };
      ui.sort = "relevance";
      navigate("results");
      break;
    case "return-to-workspace": {
      const filter = ui.results.originFilter;
      openWorkspace(ui.workspace);
      ui.filter = filter;
      renderPage();
      break;
    }
    case "home-tab":
      ui.homeTab = id;
      ui.selected.clear();
      renderPage();
      break;
    case "filter":
      ui.filter = id;
      ui.selected.clear();
      renderPage();
      break;
    case "new-workspace":
    case "new-workspace-for":
      formDialog("workspace", "新建工作区", field("name", "工作区名称", "", "例如 客户交付"), "创建工作区", "", id ? id.split(",") : []);
      break;
    case "rename-workspace":
      formDialog("rename", "重命名工作区", field("name", "工作区名称", workspaceById(id).name), "保存", "", [id]);
      break;
    case "delete-workspace":
      formDialog("delete", "删除工作区", `<p>将删除「${escapeHtml(workspaceById(id).name)}」及其中的关联。原文件和已保存的网页会保留。</p>`, "删除工作区", "", [id]);
      break;
    case "new-web":
      openWebForm(id);
      break;
    case "edit-web":
      openWebForm(null, id);
      break;
    case "open-resource":
      openResource(id);
      break;
    case "details":
      openDetails(resourceById(id));
      break;
    case "add-resource":
      ui.resumeSearch = Boolean(control?.closest("#search-panel"));
      addResourceDialog([id]);
      break;
    case "add-selected":
      addResourceDialog([...ui.selected]);
      break;
    case "request-unlink":
      requestUnlink([id]);
      break;
    case "remove-selected":
      requestUnlink([...ui.selected]);
      break;
    case "clear-selection":
      ui.selected.clear();
      renderPage();
      break;
    case "favorite": {
      const favorited = state.favorites.includes(id);
      state.favorites = favorited ? state.favorites.filter((item) => item !== id) : [...state.favorites, id];
      saveState();
      renderPage();
      notify(favorited ? "已取消收藏" : "已收藏");
      break;
    }
    case "locations":
      showLocations();
      break;
    case "load-demo":
      loadDemo();
      break;
    case "choose-directory":
      $("#directory-picker").click();
      break;
    case "toggle-location": {
      const location = state.locations.find((item) => item.id === id);
      if (!location.active && !location.demo) {
        showDialog("重新选择目录", `<p>请重新选择「${escapeHtml(location.name)}」。浏览器只能取得相对路径，已保存的文件关联会保留为待核对状态。</p>
          <div class="dialog-actions">${button("选择目录", "reselect-directory", "folder", "primary", id)}</div>`);
        return;
      }
      location.active = !location.active;
      ui.selected.clear();
      saveState();
      render();
      showLocations();
      notify(location.active ? "已授权示例目录" : "已撤销位置授权");
      break;
    }
    case "reselect-directory":
      $("#directory-picker").dataset.location = id;
      $("#directory-picker").click();
      break;
    case "clear-demo":
      formDialog("clear-demo", "清空示例数据", "<p>示例目录、示例资源和示例工作区将从原型中移除。</p>", "清空示例数据");
      break;
    case "repair": {
      if (id !== "archive") {
        showDialog("重新指向原件", "<p>真实文件的身份核对由桌面版完成。当前工作区保留了这条关联。</p>");
        break;
      }
      const affected = state.workspaces.filter((workspace) => workspace.resources.includes(id)).map((workspace) => workspace.name);
      formDialog("repair", "重新指向示例文件",
        `<label class="choice"><input type="radio" name="replacement" value="${demoRepairPath}">${demoRepairPath}</label>
         <p>${affected.length ? `更新后会同步到「${escapeHtml(affected.join("、"))}」。` : "更新后可从全局搜索找到该文件。"}</p>`,
        "更新引用", "", [id]);
      break;
    }
    case "locate": {
      const resource = resourceById(id);
      if (!resource || !authorized(resource)) return;
      showDialog("显示原始位置", `<p>${escapeHtml(resource.path)}</p><p>桌面版会在系统文件管理器中定位。</p><div class="dialog-actions">${button("完成", "close-dialog", "check", "primary")}</div>`);
      break;
    }
    case "close-dialog":
      dialog.close();
      break;
  }
}

function importDirectory(input) {
  const files = Array.from(input.files);
  const reauthorizeId = input.dataset.location;
  delete input.dataset.location;
  if (!files.length) return;
  const name = files[0].webkitRelativePath.split("/")[0];
  let location = reauthorizeId ? state.locations.find((item) => item.id === reauthorizeId) : null;
  if (location && location.name !== name) {
    notify(`请重新选择「${location.name}」目录`);
    return;
  }
  if (!location) {
    location = { id: makeId(), name, active: true, demo: false };
    state.locations.push(location);
  }
  location.active = true;
  // 浏览器没有原件标识，用保留的引用展示失联，避免同路径的新文件接管旧关联。
  if (reauthorizeId) {
    const keptIds = new Set([...state.favorites, ...state.workspaces.flatMap((workspace) => workspace.resources), ...state.events.map((event) => event.resourceId)]);
    state.resources = state.resources.filter((resource) => resource.rootId !== location.id || keptIds.has(resource.id));
    for (const resource of state.resources) {
      if (resource.rootId === location.id) resource.missing = true;
    }
  }
  for (const file of files) {
    state.resources.push({ id: makeId(), type: "file", rootId: location.id, path: file.webkitRelativePath, name: file.name, time: file.lastModified, size: file.size, missing: false });
  }
  saveState();
  render();
  showLocations();
  notify(`已索引 ${files.length} 个文件`);
}

search.addEventListener("focus", openSearch);
search.addEventListener("click", openSearch);
search.addEventListener("input", () => {
  ui.resultIndex = -1;
  if (ui.searchOpen) renderSearch();
  else openSearch();
});
search.addEventListener("keydown", (event) => {
  if (event.isComposing || event.keyCode === 229) return;
  if (event.key === "ArrowDown" || event.key === "ArrowUp") {
    event.preventDefault();
    openSearch();
    const count = ui.quickIds.length;
    if (!count) return;
    const direction = event.key === "ArrowDown" ? 1 : -1;
    const start = ui.resultIndex === -1 && direction === -1 ? 0 : ui.resultIndex;
    ui.resultIndex = (start + direction + count) % count;
    updateActiveResult(true);
  }
  if (event.key === "Enter" && ui.searchOpen && ui.resultIndex >= 0) {
    event.preventDefault();
    const item = ui.quickIds[ui.resultIndex];
    handleAction(item.type === "workspace" ? "open-workspace" : "open-resource", item.id);
  }
});
document.addEventListener("keydown", (event) => {
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k" && !dialog.open) {
    event.preventDefault();
    focusSearch();
  }
  if (event.key === "Escape" && !event.isComposing && ui.searchOpen && !dialog.open) {
    event.preventDefault();
    dismissSearch();
    ui.searchIntent = "browse";
    $("#page").focus({ preventScroll: true });
  }
});
document.addEventListener("pointerdown", (event) => {
  if (!event.target.closest("#search-area") && ui.searchOpen) {
    dismissSearch();
    ui.searchIntent = "browse";
  }
});
document.addEventListener("click", (event) => {
  const control = event.target.closest("[data-action]");
  if (control) handleAction(control.dataset.action, control.dataset.id || "", control);
});
document.addEventListener("dblclick", (event) => {
  const row = event.target.closest("tr[data-resource]");
  if (row && !event.target.closest("input, button")) openResource(row.dataset.resource);
});
document.addEventListener("change", (event) => {
  const control = event.target;
  if (control.dataset.select === "scope") {
    ui.scope = control.value;
    ui.resultIndex = -1;
    renderSearch();
    search.focus();
  } else if (control.dataset.select) {
    ui[control.dataset.select] = control.value;
    renderPage();
  } else if (control.dataset.check) {
    const checked = control.checked;
    const id = control.dataset.check;
    const visibleIds = id === "all" ? [...control.closest("table").querySelectorAll("tbody input:enabled")].map((input) => input.dataset.check) : [id];
    for (const resourceId of visibleIds) {
      if (checked) ui.selected.add(resourceId);
      else ui.selected.delete(resourceId);
    }
    renderPage();
    $("#page").querySelector(`input[data-check="${id}"]`)?.focus({ preventScroll: true });
  }
});
dialog.addEventListener("submit", handleForm);
dialog.addEventListener("close", () => {
  if (!ui.resumeSearch || dialog.open) return;
  ui.resumeSearch = false;
  search.focus({ preventScroll: true });
  openSearch();
});
$("#directory-picker").addEventListener("change", (event) => {
  importDirectory(event.target);
  event.target.value = "";
});
$("#directory-picker").addEventListener("cancel", (event) => { delete event.target.dataset.location; });

let draggedId = null;
document.addEventListener("dragstart", (event) => {
  const row = event.target.closest("tr[draggable='true']");
  if (!row) return;
  draggedId = row.dataset.resource;
  event.dataTransfer.setData("application/x-workspace-resource", draggedId);
  event.dataTransfer.effectAllowed = "link";
});
document.addEventListener("dragover", (event) => {
  const drop = event.target.closest("[data-drop-workspace]");
  if (!draggedId || !drop) return;
  event.preventDefault();
  event.dataTransfer.dropEffect = "link";
  drop.classList.add("drop-target");
});
document.addEventListener("dragleave", (event) => {
  const drop = event.target.closest("[data-drop-workspace]");
  if (drop && !drop.contains(event.relatedTarget)) drop.classList.remove("drop-target");
});
document.addEventListener("drop", (event) => {
  const drop = event.target.closest("[data-drop-workspace]");
  const id = event.dataTransfer.getData("application/x-workspace-resource");
  if (!drop || !id || id !== draggedId) return;
  event.preventDefault();
  drop.classList.remove("drop-target");
  linkResources(ui.selected.has(id) ? [...ui.selected] : [id], [workspaceById(drop.dataset.dropWorkspace)]);
});
document.addEventListener("dragend", () => {
  draggedId = null;
  document.querySelectorAll(".drop-target").forEach((target) => target.classList.remove("drop-target"));
});

$("#search-icon").innerHTML = icon("search");
$("#search-shortcut").textContent = shortcut;
$("#locations-top").innerHTML = `${icon("folder")}管理位置`;
render();

