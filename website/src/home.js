import messages from './home.generated.js';
import './download.js';

const t = (text) => document.documentElement.lang === 'en' ? messages[text] : text;
const toast = document.getElementById('toast');
let toastTimer;
function showToast(message) {
  toast.textContent = message;
  toast.classList.add('show');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => toast.classList.remove('show'), 2600);
}

const header = document.getElementById('siteHeader');
const menuToggle = document.getElementById('menuToggle');
function closeMenu() {
  header.classList.remove('menu-open');
  menuToggle.setAttribute('aria-expanded', 'false');
  menuToggle.textContent = '☰';
}
menuToggle.addEventListener('click', () => {
  const open = header.classList.toggle('menu-open');
  menuToggle.setAttribute('aria-expanded', String(open));
  menuToggle.textContent = open ? '×' : '☰';
});
header.querySelectorAll('a').forEach((link) => link.addEventListener('click', closeMenu));
header.addEventListener('keydown', (event) => {
  if (event.key === 'Escape' && header.classList.contains('menu-open')) {
    closeMenu();
    menuToggle.focus();
  }
});

// 原生滚动不接管滚轮，进度只在下一帧更新合成属性。
const scrollLine = document.getElementById('scrollLine');
let scrollFrame = 0;
function updateScroll() {
  const max = document.documentElement.scrollHeight - window.innerHeight;
  scrollLine.style.transform = `scaleX(${max > 0 ? window.scrollY / max : 0})`;
  scrollFrame = 0;
}
function scheduleScroll() {
  if (!scrollFrame) scrollFrame = requestAnimationFrame(updateScroll);
}
window.addEventListener('scroll', scheduleScroll, { passive: true });
window.addEventListener('resize', scheduleScroll);
updateScroll();

// 脚本就绪后才准备渐入；提前少量触发，文字与示意卡错峰呈现，退出不重播。
if (!window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
  const revealObserver = new IntersectionObserver((entries) => {
    for (const entry of entries) {
      if (!entry.isIntersecting) continue;
      entry.target.classList.remove('reveal-pending');
      entry.target.classList.add('is-revealed');
      revealObserver.unobserve(entry.target);
    }
  }, { threshold: 0.08, rootMargin: '0px 0px -48px 0px' });
  document.querySelectorAll('.reveal').forEach((element) => {
    element.classList.add('reveal-pending');
    revealObserver.observe(element);
  });
}

const demoInput = document.getElementById('demoInput');
const demoResults = document.getElementById('demoResults');
const demoData = [
  { name: t("品牌企划"), desc: t("工作区 · 12 份资料"), type: t("工作区"), symbol: '▰', keys: [t("品牌"), t("企划")] },
  { name: t("品牌企划预算.xlsx"), desc: t("文档 / 项目 / 预算"), type: t("文件"), symbol: 'X', keys: [t("品牌"), t("企划"), t("预算")] },
  { name: t("品牌视觉参考"), desc: 'https://are.na/…', type: t("网页"), symbol: '↗', keys: [t("品牌"), t("视觉"), t("参考")] },
  { name: t("季度运营复盘.docx"), desc: t("文档 / 运营 / 复盘"), type: t("文件"), symbol: 'W', keys: [t("运营"), t("复盘")] },
  { name: t("产品需求与路线图.pdf"), desc: t("文档 / 产品 / 规划"), type: t("文件"), symbol: 'P', keys: [t("产品"), t("需求"), t("路线图")] },
];
let matches = [];
let selected = -1;
function selectResult(index) {
  selected = index;
  [...demoResults.children].forEach((row, position) => {
    row.classList.toggle('selected', position === selected);
    row.setAttribute('aria-selected', String(position === selected));
  });
  if (selected >= 0) demoInput.setAttribute('aria-activedescendant', `demo-result-${selected}`);
  else demoInput.removeAttribute('aria-activedescendant');
}
function openResult(index) {
  const item = matches[index];
  if (!item) return;
  selectResult(index);
  // 官网只反馈所选示例，不打开本地文件或外部网页。
  showToast(t("演示：打开「") + item.name + (document.documentElement.lang === 'en' ? '”' : '」'));
}
function renderDemoResults() {
  const query = demoInput.value.trim().toLocaleLowerCase();
  matches = demoData.filter((item) => !query || `${item.name} ${item.desc}`.toLocaleLowerCase().includes(query) || item.keys.some((key) => key.toLocaleLowerCase().includes(query) || query.includes(key.toLocaleLowerCase()))).slice(0, 4);
  demoResults.replaceChildren();
  for (const [index, item] of matches.entries()) {
    const row = document.createElement('div');
    row.className = 'demo-result';
    row.id = `demo-result-${index}`;
    row.setAttribute('role', 'option');
    row.tabIndex = 0;
    const symbol = document.createElement('span');
    symbol.className = 'result-symbol';
    symbol.textContent = item.symbol;
    const description = document.createElement('span');
    const name = document.createElement('strong');
    name.textContent = item.name;
    const location = document.createElement('small');
    location.textContent = item.desc;
    description.append(name, location);
    const kind = document.createElement('span');
    kind.className = 'kind';
    kind.textContent = item.type;
    row.append(symbol, description, kind);
    row.addEventListener('click', () => openResult(index));
    row.addEventListener('focus', () => selectResult(index));
    row.addEventListener('keydown', (event) => {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        openResult(index);
      }
      if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
        event.preventDefault();
        const next = (index + (event.key === 'ArrowDown' ? 1 : -1) + matches.length) % matches.length;
        demoResults.children[next].focus();
      }
      if (event.key === 'Escape') demoInput.focus();
    });
    demoResults.append(row);
  }
  selectResult(matches.length ? 0 : -1);
  if (!matches.length) {
    const empty = document.createElement('p');
    empty.className = 'demo-empty';
    empty.setAttribute('role', 'status');
    empty.textContent = t("没有匹配的演示结果");
    demoResults.append(empty);
  }
  scheduleScroll();
}
demoInput.addEventListener('input', renderDemoResults);
demoInput.addEventListener('keydown', (event) => {
  if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault();
    if (matches.length) selectResult((selected + (event.key === 'ArrowDown' ? 1 : -1) + matches.length) % matches.length);
  }
  if (event.key === 'Enter') { event.preventDefault(); openResult(selected); }
  if (event.key === 'Escape') { demoInput.value = ''; renderDemoResults(); }
});
renderDemoResults();

// 首屏入口复用正文真实演示，不再反馈不存在的搜索面板。
const heroSearch = document.getElementById('heroSearch');
function focusSearch() {
  demoInput.focus({ preventScroll: true });
  document.getElementById('searchDemo').scrollIntoView({ block: 'center' });
}
heroSearch.addEventListener('click', focusSearch);
heroSearch.addEventListener('keydown', (event) => {
  if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); focusSearch(); }
});
document.querySelectorAll('#appWindow, #searchDemo').forEach((area) => area.addEventListener('keydown', (event) => {
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') { event.preventDefault(); focusSearch(); }
}));

// 使用原生 hidden 控制完整内容，不以固定高度截断英文或放大文字。
for (const button of document.querySelectorAll('.faq-question')) {
  button.nextElementSibling.hidden = true;
  button.setAttribute('aria-expanded', 'false');
  button.addEventListener('click', () => {
    const open = button.getAttribute('aria-expanded') !== 'true';
    for (const other of document.querySelectorAll('.faq-question')) {
      const active = other === button && open;
      other.setAttribute('aria-expanded', String(active));
      other.closest('.faq-item').classList.toggle('open', active);
      other.nextElementSibling.hidden = !active;
    }
    scheduleScroll();
  });
}
const windowsDownload = document.getElementById('windowsDownload');
if (windowsDownload.tagName === 'BUTTON') windowsDownload.addEventListener('click', () => showToast(t("Windows 安装包尚未发布。")));
