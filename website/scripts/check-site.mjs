import assert from 'node:assert/strict';
import { readFile, stat } from 'node:fs/promises';
import { JSDOM } from 'jsdom';
import { origin, pages, locales, localizedRoute } from '../site.config.mjs';

const dist = new URL('../dist/', import.meta.url);
const routes = Object.values(pages).filter((page) => page.route !== '/404').flatMap((page) => locales.map((locale) => localizedRoute(locale, page.route)));
const fileFor = (route) => route.endsWith('/') ? `${route.slice(1)}index.html` : `${route.slice(1)}.html`;
const documents = new Map();
for (const route of [...routes, '/404', '/en/404']) {
  documents.set(route, new JSDOM(await readFile(new URL(fileFor(route), dist), 'utf8')).window.document);
}
const sitemap = new JSDOM(await readFile(new URL('sitemap.xml', dist), 'utf8'), { contentType: 'application/xml' }).window.document;
const indexed = [...sitemap.querySelectorAll('url > loc')].map((node) => node.textContent);
assert.deepEqual(indexed.sort(), routes.map((route) => origin + route).sort());

for (const route of routes) {
  const document = documents.get(route);
  const english = route.startsWith('/en/');
  const counterpart = english ? route.slice(3) : `/en${route}`;
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
  assert.equal(document.querySelector('meta[property="og:url"]').content, origin + route);
  const image = document.querySelector('meta[property="og:image"]').content;
  const png = await readFile(new URL(new URL(image).pathname.slice(1), dist));
  assert.equal(png.readUInt32BE(16), 1200);
  assert.equal(png.readUInt32BE(20), 630);
  const graph = JSON.parse(document.querySelector('script[type="application/ld+json"]').textContent)['@graph'];
  assert.ok(graph.some((item) => item['@type'] === 'WebPage' && item.url === origin + route));
  assert.ok(!graph.some((item) => item.aggregateRating || item.offers), '不得虚构评分或定价');
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
    assert.ok(document.querySelector('[data-release-download]').getAttribute('href').startsWith('/downloads/'));
    assert.equal(document.querySelector('[data-release="sha256"]').textContent.length, 64);
    assert.ok(document.querySelectorAll('.fact-list > div').length >= 6);
  }
  assert.ok(document.querySelector('.resource-library a[href]'), '内容导航必须出现在原始HTML中');
  if (english) {
    const content = document.body.cloneNode(true);
    content.querySelectorAll('.language-switch').forEach((element) => element.remove());
    assert.ok(!/\p{Script=Han}/u.test(content.textContent), `英文页面残留未翻译正文：${route}`);
  }
  if (route === '/' || route === '/en/') {
    assert.equal(document.querySelectorAll('#demo-resources .resource-row').length, 4, '禁用JS时也应有首屏示例');
    assert.ok(document.querySelector('#release-download').getAttribute('href').startsWith('/downloads/'), '下载不能依赖JS赋值');
    assert.equal(document.querySelector('#release-sha').textContent.length, 64);
  }
  console.log(`通过：${route} 的语言、SEO、互链、分享图、结构化数据与站内链接`);
}
for (const route of ['/404', '/en/404']) {
  assert.equal(documents.get(route).querySelector('meta[name="robots"]').content, 'noindex');
}
const llms = await readFile(new URL('llms.txt', dist), 'utf8');
for (const route of routes) assert.ok(llms.includes(origin + route));
console.log(`通过：${routes.length}个可索引页面与双语404；正文、下载和导航均在原始HTML中。`);
