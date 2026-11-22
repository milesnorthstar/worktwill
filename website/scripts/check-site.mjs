import assert from 'node:assert/strict';
import { readFile, stat } from 'node:fs/promises';
import { JSDOM } from 'jsdom';
import { origin, pages, locales, localizedRoute, repository } from '../site.config.mjs';
import {currentRelease,downloadUrl,platformNames} from '../release.mjs';
const release=currentRelease(JSON.parse(await readFile(new URL('../release.json',import.meta.url),'utf8')));

const dist = new URL('../dist/', import.meta.url);
const routes = Object.values(pages).filter((page) => page.route !== '/404').flatMap((page) => locales.map((locale) => localizedRoute(locale, page.route)));
const fileFor = (route) => route.endsWith('/') ? `${route.slice(1)}index.html` : `${route.slice(1)}.html`;
const documents = new Map();
for (const route of [...routes, '/404', '/zh/404']) {
  documents.set(route, new JSDOM(await readFile(new URL(fileFor(route), dist), 'utf8')).window.document);
}
const sitemap = new JSDOM(await readFile(new URL('sitemap.xml', dist), 'utf8'), { contentType: 'application/xml' }).window.document;
const indexed = [...sitemap.querySelectorAll('url > loc')].map((node) => node.textContent);
assert.deepEqual(indexed.sort(), routes.map((route) => origin + route).sort());

for (const route of routes) {
  const document = documents.get(route);
  const english = !route.startsWith('/zh/');
  const counterpart = english ? `/zh${route}` : route.slice(3);
  assert.equal(document.documentElement.lang, english ? 'en' : 'zh-CN', route);
  assert.equal(document.querySelectorAll('h1').length, 1, route);
  assert.ok(document.querySelector('meta[name="description"]').content.length > 30, route);
  assert.equal(document.querySelector('link[rel="canonical"]').href, origin + route, route);
  const alternates = [...document.querySelectorAll('link[rel="alternate"]')];
  assert.equal(alternates.length, 3, route);
  for (const alternate of alternates) {
    const target = documents.get(new URL(alternate.href).pathname);
    assert.ok(target, `hreflang指向不存在的页面：${alternate.href}`);
    assert.ok([...target.querySelectorAll('link[rel="alternate"]')].some((node) => node.href === origin + route), `缺少互相回指：${route}`);
  }
  assert.equal(document.querySelector(`.language-switch a[hreflang="${english ? 'zh-CN' : 'en'}"]`).getAttribute('href'), counterpart, route);
  assert.equal(document.querySelector('link[hreflang="x-default"]').href, origin + (english ? route : route.slice(3)));
  assert.equal(document.querySelector('meta[property="og:url"]').content, origin + route);
  const image = document.querySelector('meta[property="og:image"]').content;
  const png = await readFile(new URL(new URL(image).pathname.slice(1), dist));
  assert.equal(png.readUInt32BE(16), 1200);
  assert.equal(png.readUInt32BE(20), 630);
  const graph = JSON.parse(document.querySelector('script[type="application/ld+json"]').textContent)['@graph'];
  assert.ok(graph.some((item) => item['@type'] === 'WebPage' && item.url === origin + route));
  assert.ok(!graph.some((item) => item.aggregateRating || item.offers), '不得虚构评分或定价');
  const meta = Object.values(pages).find((page) => localizedRoute(english ? 'en' : 'zh-CN', page.route) === route);
  const pageSchema = graph.find((item) => item['@type'] === 'WebPage');
  assert.equal(pageSchema.dateModified, meta.updated);
  const sitemapEntry = [...sitemap.querySelectorAll('url')].find((entry) => entry.querySelector('loc').textContent === origin + route);
  assert.equal(sitemapEntry.querySelector('lastmod')?.textContent, meta.updated);
  const application = graph.find((item) => item['@type'] === 'SoftwareApplication');
  if (application) {
    assert.equal(application.sameAs, repository);
    assert.equal(application.softwareVersion, release.version);
  }
  assert.ok(!/开源计划|open-source plans|候选 026|Candidate 026/.test(document.documentElement.textContent), `${route} 含过时发布或开源计划文案`);
  for (const anchor of document.querySelectorAll('a[href]')) {
    const url = new URL(anchor.getAttribute('href'), origin + route);
    if (url.origin !== origin) continue;
    if (url.pathname.startsWith('/downloads/')) {
      assert.ok((await stat(new URL(url.pathname.slice(1), dist))).size > 0);
      continue;
    }
    const target = documents.get(url.pathname);
    assert.ok(target, `无效站内链接：${route} -> ${url.pathname}`);
    if (url.hash) assert.ok(target.getElementById(decodeURIComponent(url.hash.slice(1))), `无效锚点：${url.href}`);
  }
  if (route.endsWith('/download')) {
    assert.equal(document.querySelector('[data-release-download]').getAttribute('href'),downloadUrl(release));
    assert.equal(document.querySelectorAll('[data-platform-downloads] .platform-download').length,Object.keys(platformNames).length-1);
    assert.equal(document.querySelectorAll('[data-platform-downloads] a').length,Object.keys(release.platforms).length-1);
    assert.equal(document.querySelector('[data-release="sha256"]').textContent.length, 64);
    assert.ok(document.querySelectorAll('.fact-list > div').length >= 6);
  }
  assert.ok(document.querySelector('.resource-library a[href], .footer-links a[href="/guide"], .footer-links a[href="/zh/guide"]'), '内容导航必须出现在原始HTML中');
  assert.equal(document.querySelector('.nav-github').href, 'https://github.com/milesnorthstar/worktwill');
  assert.ok(!document.body.textContent.includes('公开源码仓库和具体开源许可证尚未发布'));
  assert.ok(!document.body.textContent.includes('public repository, license, and contribution rules are still to be published'));
  if (route !== '/' && route !== '/zh/') {
    assert.ok(document.body.classList.contains('document-site'));
    assert.ok(document.querySelector('.document-hero h1'));
    assert.ok(document.querySelector('.site-header .brand-mark svg'));
    assert.ok(document.querySelector('footer .footer-row'));
    assert.ok(!document.querySelector('link[href="/src/style.css"]'));
  }
  if (english) {
    const content = document.body.cloneNode(true);
    content.querySelectorAll('.language-switch').forEach((element) => element.remove());
    assert.ok(!/\p{Script=Han}/u.test(content.textContent), `英文页面残留未翻译正文：${route}`);
  }
  if (route === '/' || route === '/zh/') {
    // 保留原型的窗口、三条静态示例和平台卡片，不再混入旧官网组件。
    assert.equal(document.querySelectorAll('#appWindow .resource-row').length, 3);
    assert.equal(document.querySelector('#macDownload').getAttribute('href'), downloadUrl(release.platforms['darwin-aarch64']));
    assert.equal(document.querySelector('#macDownload').title.length, 73);
    assert.ok(!document.querySelector('link[href="/src/style.css"]'));
    // 原型占位入口不能进入正式双语页面。
    assert.ok(!document.querySelector('#opensource, a[href="#opensource"]'));
    assert.equal(document.querySelector('.nav-github').textContent.trim(), '');
    assert.equal(document.querySelector('.nav-github').getAttribute('aria-label'), 'GitHub');
    assert.equal(document.querySelector('#demoInput').value, english ? 'brand plan' : '品牌企划');
    assert.equal(document.querySelectorAll('.faq-question').length, 5);
    assert.equal(document.querySelector('#menuToggle').getAttribute('aria-controls'), 'home-navigation');
    assert.ok(!document.querySelector('.nav-actions .language-switch'), '手机语言入口不能藏在操作区');
    assert.equal(document.querySelector('#demoInput').getAttribute('aria-controls'), 'demoResults');
    assert.equal(document.querySelector('#demoResults').getAttribute('role'), 'listbox');
    assert.ok(!document.querySelector('#cursorGlow'));
    assert.ok(!document.querySelector('a[href="https://github.com/"]'));
    assert.ok(!document.body.textContent.includes('v0.1.0'));
  }
  console.log(`通过：${route} 的语言、SEO、互链、分享图、结构化数据与站内链接`);
}
for (const route of ['/404', '/zh/404']) {
  assert.equal(documents.get(route).querySelector('meta[name="robots"]').content, 'noindex');
}
// 正式交互回归直接执行首页源码，覆盖初始结果与输入后的同一条路径。
const homeSource = await readFile(new URL('../src/home.js', import.meta.url), 'utf8');
const messageSource = await readFile(new URL('../src/home.generated.js', import.meta.url), 'utf8');
for (const route of ['/', '/zh/']) {
  const dom = new JSDOM(documents.get(route).documentElement.outerHTML, { runScripts: 'outside-only', pretendToBeVisual: true });
  const { window } = dom;
  const document = window.document;
  let focusedDemo = false;
  document.querySelector('#searchDemo').scrollIntoView = () => { focusedDemo = true; };
  let revealCallback;
  const observed = new Set();
  window.matchMedia = () => ({ matches: false });
  window.IntersectionObserver = class {
    constructor(callback) { revealCallback = callback; }
    observe(element) { observed.add(element); }
    unobserve(element) { observed.delete(element); }
  };
  window.eval(homeSource.replace("import messages from './home.generated.js';", messageSource.replace('export default', 'const messages =')));
  const reveal = document.querySelector('.reveal');
  assert.ok(observed.has(reveal));
  assert.ok(reveal.classList.contains('reveal-pending'));
  revealCallback([{ target: reveal, isIntersecting: false }]);
  assert.ok(!reveal.classList.contains('is-revealed'));
  revealCallback([{ target: reveal, isIntersecting: true }]);
  assert.ok(reveal.classList.contains('is-revealed'));
  assert.ok(!reveal.classList.contains('reveal-pending'));
  assert.ok(!observed.has(reveal), '入场后停止观察，避免滚动反复重播');
  const input = document.querySelector('#demoInput');
  const key = (element, value) => element.dispatchEvent(new window.KeyboardEvent('keydown', { key: value, bubbles: true }));
  assert.equal(document.querySelectorAll('#demoResults [role="option"]').length, 3, '初始搜索结果必须可操作');
  document.querySelector('#demoResults [role="option"]').click();
  assert.ok(document.querySelector('#toast').textContent.includes(input.value));
  key(input, 'ArrowDown');
  assert.equal(input.getAttribute('aria-activedescendant'), 'demo-result-1');
  key(input, 'Enter');
  assert.ok(document.querySelector('#toast').textContent.includes('xlsx'));
  input.value = 'no-matching-resource-xyz';
  input.dispatchEvent(new window.Event('input'));
  const previousToast = document.querySelector('#toast').textContent;
  assert.equal(document.querySelectorAll('#demoResults [role="option"]').length, 0);
  assert.ok(!input.hasAttribute('aria-activedescendant'));
  key(input, 'Enter');
  assert.equal(document.querySelector('#toast').textContent, previousToast, '空结果不能反馈打开成功');
  key(input, 'Escape');
  assert.equal(input.value, '');
  assert.equal(document.querySelectorAll('#demoResults [role="option"]').length, 4);
  document.querySelector('#heroSearch').click();
  assert.equal(document.activeElement, input);
  assert.ok(focusedDemo);
  const question = document.querySelectorAll('.faq-question')[1];
  assert.ok(question.nextElementSibling.hidden);
  question.click();
  assert.equal(question.getAttribute('aria-expanded'), 'true');
  assert.equal(question.nextElementSibling.hidden, false);
  document.querySelector('.faq-question').click();
  assert.ok(question.nextElementSibling.hidden);
  document.querySelector('#menuToggle').click();
  key(document.querySelector('#menuToggle'), 'Escape');
  assert.equal(document.querySelector('#menuToggle').getAttribute('aria-expanded'), 'false');
  dom.window.close();
  console.log(`通过：${route} 的搜索初始/键盘/空结果、首屏聚焦、FAQ与菜单交互`);
}
// 减少动态设置下不创建观察器，正文无需解除隐藏即可阅读。
{
  const dom = new JSDOM(documents.get('/').documentElement.outerHTML, { runScripts: 'outside-only', pretendToBeVisual: true });
  dom.window.matchMedia = () => ({ matches: true });
  dom.window.IntersectionObserver = class { constructor() { assert.fail('减少动态时不应注册动效观察'); } };
  dom.window.eval(homeSource.replace("import messages from './home.generated.js';", messageSource.replace('export default', 'const messages =')));
  assert.equal(dom.window.document.querySelectorAll('.is-revealed, .reveal-pending').length, 0);
  dom.window.close();
}
const redirects = await readFile(new URL('_redirects', dist), 'utf8');
for (const { route } of Object.values(pages).filter((page) => page.route !== '/404')) {
  assert.ok(redirects.split('\n').includes(`/en${route} ${route} 301`));
}
const llms = await readFile(new URL('llms.txt', dist), 'utf8');
for (const route of routes) assert.ok(llms.includes(origin + route));
assert.ok(llms.includes(repository));
assert.ok(llms.includes(`version ${release.version}`));
assert.ok(!/not yet published|candidate 026/i.test(llms));
console.log(`通过：${routes.length}个可索引页面与双语404；正文、下载和导航均在原始HTML中。`);
