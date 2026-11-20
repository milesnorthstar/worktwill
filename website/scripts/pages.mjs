import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { JSDOM } from 'jsdom';
import demo from '../locales/demo.mjs';
import { origin, locales, pages, localizedRoute, bingVerification } from '../site.config.mjs';

const site = new URL('../', import.meta.url);
const english = {
  ...JSON.parse(await readFile(new URL('locales/en.json', site), 'utf8')),
  ...JSON.parse(await readFile(new URL('locales/geo-en.json', site), 'utf8')),
};
const release = JSON.parse(await readFile(new URL('release.json', site), 'utf8'));
const indexnow = JSON.parse(await readFile(new URL('indexnow.json', site), 'utf8'));
if (!/^[a-zA-Z0-9-]{8,128}$/.test(indexnow.key)) throw new Error('IndexNow公开站点校验键格式无效');
await writeFile(new URL(`public/${indexnow.key}.txt`, site), indexnow.key);
const hasChinese = (value) => /\p{Script=Han}/u.test(value);

function translate(value) {
  if (!hasChinese(value)) return value;
  const source = value.trim();
  if (!Object.hasOwn(english, source)) throw new Error(`缺少英文翻译：${source}`);
  return value.replace(source, english[source]);
}

function translateData(value) {
  if (typeof value === 'string') return translate(value);
  if (Array.isArray(value)) return value.map(translateData);
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, translateData(item)]));
  return value;
}

const demos = { 'zh-CN': demo, en: translateData(demo) };
// 客户端只加载演示所需文案，不把整篇使用/隐私词典带入脚本。
await writeFile(new URL('src/demo.generated.js', site), `// 由 scripts/pages.mjs 生成，请修改 locales/ 中的来源。\nexport default ${JSON.stringify(demos)};\n`);

function setMeta(document, name, content, property = false) {
  const attribute = property ? 'property' : 'name';
  let element = document.querySelector(`meta[${attribute}="${name}"]`);
  if (!element) {
    element = document.createElement('meta');
    element.setAttribute(attribute, name);
    document.head.append(element);
  }
  element.content = content;
}

for (const [page, meta] of Object.entries(pages)) {
  const source = await readFile(new URL(`templates/${page}.html`, site), 'utf8');
  for (const locale of locales) {
    const dom = new JSDOM(source);
    const { document, NodeFilter } = dom.window;
    // 内容导航进入静态HTML，保证新页面能从首页和说明页被发现。
    const resources = document.createElement('nav');
    resources.className = 'resource-library section-wrap';
    resources.setAttribute('aria-label', '产品资料');
    const resourceHeading = document.createElement('h2');
    resourceHeading.textContent = '了解更多，让资料有处可寻。';
    resources.append(resourceHeading);
    for (const [path, label] of [['/about', '产品事实与开源计划'], ['/contract-workspace', '客户合同整理步骤'], ['/download', '下载、版本与支持'], ['/guide', '安装与使用说明'], ['/privacy', '隐私说明']]) {
      const link = document.createElement('a');
      link.href = path;
      link.textContent = label;
      resources.append(link);
    }
    document.querySelector('footer')?.before(resources);
    document.documentElement.lang = locale;
    if (locale === 'en') {
      const walker = document.createTreeWalker(document.documentElement, NodeFilter.SHOW_TEXT);
      while (walker.nextNode()) walker.currentNode.textContent = translate(walker.currentNode.textContent);
      for (const element of document.querySelectorAll('*')) {
        for (const attribute of ['aria-label', 'placeholder', 'alt', 'content']) {
          if (element.hasAttribute(attribute)) element.setAttribute(attribute, translate(element.getAttribute(attribute)));
        }
      }
    }

    for (const anchor of document.querySelectorAll('a[href]')) {
      const href = anchor.getAttribute('href');
      if (!href.startsWith('/') || href.startsWith('//')) continue;
      const [path, fragment] = href.split('#');
      const route = path.replace(/\.html$/, '');
      if (Object.values(pages).some((page) => page.route === route)) {
        anchor.setAttribute('href', localizedRoute(locale, route) + (fragment ? `#${fragment}` : ''));
      }
    }

    const actions = document.createElement('div');
    actions.className = 'header-actions';
    const languages = document.createElement('nav');
    languages.className = 'language-switch';
    languages.setAttribute('aria-label', locale === 'en' ? 'Language' : '语言');
    for (const language of locales) {
      const link = document.createElement('a');
      link.href = localizedRoute(language, meta.route);
      link.lang = language;
      link.hreflang = language;
      link.textContent = language === 'en' ? 'EN' : '中文';
      link.setAttribute('aria-label', language === 'en' ? 'English' : '简体中文');
      if (language === locale) link.setAttribute('aria-current', 'page');
      languages.append(link);
    }
    const header = document.querySelector('header');
    actions.append(languages);
    const cta = header.querySelector('.nav-download');
    if (cta) actions.append(cta);
    header.append(actions);

    for (const element of document.querySelectorAll('[data-updated]')) {
      element.setAttribute('datetime', meta.updated);
      element.textContent = meta.updated;
    }
    for (const element of document.querySelectorAll('[data-release]')) {
      const field = element.getAttribute('data-release');
      const value = field === 'stage' && locale === 'en' ? 'V1 acceptance candidate · 026' : field === 'bytes' ? `${release.bytes.toLocaleString(locale)} bytes (6.3 MiB)` : release[field];
      if (value === undefined) throw new Error(`未知发行字段：${field}`);
      element.textContent = value;
    }
    for (const element of document.querySelectorAll('[data-release-download]')) element.href = `/downloads/${release.filename}`;

    const canonical = origin + localizedRoute(locale, meta.route);
    document.title = locale === 'en' ? meta.enTitle : meta.title;
    if (page === 'index') setMeta(document, 'msvalidate.01', bingVerification);
    document.querySelectorAll('link[rel="canonical"], link[rel="alternate"]').forEach((element) => element.remove());
    if (page !== '404') {
      const link = document.createElement('link');
      link.rel = 'canonical';
      link.href = canonical;
      document.head.append(link);
      for (const language of [...locales, 'x-default']) {
        const alternate = document.createElement('link');
        alternate.rel = 'alternate';
        alternate.hreflang = language;
        alternate.href = origin + localizedRoute(language === 'x-default' ? 'zh-CN' : language, meta.route);
        document.head.append(alternate);
      }
      const description = document.querySelector('meta[name="description"]').content;
      const image = `${origin}/og/${locale === 'en' ? 'en' : 'zh'}.png`;
      for (const [property, content] of Object.entries({
        'og:type': 'website', 'og:site_name': 'WorkTwill', 'og:title': document.title,
        'og:description': description, 'og:url': canonical, 'og:locale': locale === 'en' ? 'en_US' : 'zh_CN',
        'og:locale:alternate': locale === 'en' ? 'zh_CN' : 'en_US',
        'og:image': image, 'og:image:width': '1200', 'og:image:height': '630',
        'og:image:alt': document.title,
      })) setMeta(document, property, content, true);
      for (const [name, content] of Object.entries({
        'twitter:card': 'summary_large_image', 'twitter:title': document.title,
        'twitter:description': description, 'twitter:image': image,
        'robots': 'index, follow, max-image-preview:large',
      })) setMeta(document, name, content);

      const graph = [
        { '@type': 'WebSite', '@id': `${origin}/#website`, name: 'WorkTwill', url: `${origin}/`, inLanguage: locales },
        { '@type': 'WebPage', '@id': `${canonical}#page`, url: canonical, name: document.title,
          description, inLanguage: locale, isPartOf: { '@id': `${origin}/#website` }, primaryImageOfPage: { '@type': 'ImageObject', url: image, width: 1200, height: 630 } },
      ];
      if (['index', 'about', 'download'].includes(page)) graph.push({
        '@type': 'SoftwareApplication', '@id': `${origin}/#application`, name: 'WorkTwill',
        url: canonical, applicationCategory: 'BusinessApplication', operatingSystem: 'macOS (Apple Silicon)',
        softwareVersion: release.version, description, inLanguage: locales,
        downloadUrl: `${origin}/downloads/${release.filename}`, releaseNotes: origin + localizedRoute(locale, '/guide'),
      });
      if (page !== 'index') graph.push({
        '@type': 'BreadcrumbList', itemListElement: [
          { '@type': 'ListItem', position: 1, name: locale === 'en' ? 'Home' : '首页', item: origin + localizedRoute(locale, '/') },
          { '@type': 'ListItem', position: 2, name: document.title, item: canonical },
        ],
      });
      if (meta.updated) {
        graph.find((item) => item['@type'] === 'WebPage').dateModified = meta.updated;
      }
      const structured = document.createElement('script');
      structured.type = 'application/ld+json';
      structured.textContent = JSON.stringify({ '@context': 'https://schema.org', '@graph': graph }).replaceAll('<', '\\u003c');
      document.head.append(structured);
    }

    if (page === 'index') {
      document.querySelector('#release-download').href = `/downloads/${release.filename}`;
      document.querySelector('#release-sha').textContent = release.sha256;
      // 首屏示例也预渲染进HTML，禁用脚本时正文和下载仍完整可用。
      const list = document.querySelector('#demo-resources');
      for (const resource of demos[locale].resources.slice(0, 4)) {
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
        row.append(name);
        for (const field of ['location', 'type']) {
          const span = document.createElement('span');
          span.className = `resource-${field}`;
          span.textContent = resource[field];
          row.append(span);
        }
        list.append(row);
      }
      const notice = document.createElement('noscript');
      notice.textContent = locale === 'en' ? 'The interactive demo needs JavaScript. Product information and downloads are available without it.' : '交互演示需要 JavaScript；产品介绍、说明和下载无需脚本即可使用。';
      document.querySelector('.app-window').after(notice);
    }
    const folder = new URL(locale === 'en' ? 'en/' : './', site);
    await mkdir(folder, { recursive: true });
    await writeFile(new URL(`${page}.html`, folder), dom.serialize());
    dom.window.close();
  }
}

const entries = Object.values(pages).filter((page) => page.route !== '/404').flatMap(({ route }) => locales.map((locale) => {
  const alternates = [...locales, 'x-default'].map((language) => `<xhtml:link rel="alternate" hreflang="${language}" href="${origin}${localizedRoute(language === 'x-default' ? 'zh-CN' : language, route)}"/>`).join('');
  const updated = Object.values(pages).find((page) => page.route === route).updated;
  return `<url><loc>${origin}${localizedRoute(locale, route)}</loc>${updated ? `<lastmod>${updated}</lastmod>` : ''}${alternates}</url>`;
}));
await writeFile(new URL('public/sitemap.xml', site), `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">\n${entries.join('\n')}\n</urlset>\n`);
const llms = `# WorkTwill\n\n> A local personal workspace for files and saved links. Resources are organized by reference; original files stay in place. File search and organization work offline without an account or AI setup.\n\nThe project is committed to becoming fully open source. The public source repository and specific license are not yet published. Current download: version ${release.version}, macOS Apple Silicon, early candidate 026; not a stable or Apple-notarized release. Windows and Intel Mac builds are not available. Search covers metadata, not document contents.\n\n## English\n${Object.values(pages).filter((page) => page.route !== '/404').map((page) => `- [${page.enTitle}](${origin}${localizedRoute('en', page.route)})`).join('\n')}\n\n## 简体中文\n${Object.values(pages).filter((page) => page.route !== '/404').map((page) => `- [${page.title}](${origin}${page.route})`).join('\n')}\n\n## Current release\n- [Download and verified support limits](${origin}/en/download)\n- SHA-256: ${release.sha256}\n\nThis file is a navigation aid, not a guarantee of AI search indexing, citations, or model training permission. Use the dated support page for release boundaries.\n`;
await writeFile(new URL('public/llms.txt', site), llms);
console.log(`已生成中英文${Object.keys(pages).length * 2}个静态页面、演示文案、多语言站点地图和llms导航。`);
