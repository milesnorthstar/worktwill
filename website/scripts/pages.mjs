import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { JSDOM } from 'jsdom';
import demo from '../locales/demo.mjs';
import { origin, locales, pages, localizedRoute, bingVerification, repository } from '../site.config.mjs';
import {currentRelease,downloadUrl,platformNames} from '../release.mjs';

const site = new URL('../', import.meta.url);
const english = {
  ...JSON.parse(await readFile(new URL('locales/en.json', site), 'utf8')),
  ...JSON.parse(await readFile(new URL('locales/geo-en.json', site), 'utf8')),
};
const release = currentRelease(JSON.parse(await readFile(new URL('release.json', site), 'utf8')));
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

// 首页动态文案从同一英文词典生成，缺失翻译在构建时失败。
const homeScript = await readFile(new URL('src/home.js', site), 'utf8');
const homeMessages = Object.fromEntries([...homeScript.matchAll(/t\(("(?:[^"\\\\]|\\\\.)*")\)/g)].map((match) => {
  const source = JSON.parse(match[1]);
  return [source, translate(source)];
}));
await writeFile(new URL('src/home.generated.js', site), `// 由 scripts/pages.mjs 生成。\nexport default ${JSON.stringify(homeMessages)};\n`);
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

const homeTemplate = new JSDOM(await readFile(new URL('templates/index.html', site), 'utf8')).window.document;

for (const [page, meta] of Object.entries(pages)) {
  const source = await readFile(new URL(`templates/${page}.html`, site), 'utf8');
  for (const locale of locales) {
    const dom = new JSDOM(source);
    const { document, NodeFilter } = dom.window;
    if (page !== 'index') {
      // 构建期复用首页品牌和页脚，内页无需加载搜索演示的运行脚本。
      document.body.classList.add('document-site');
      const header = homeTemplate.querySelector('header').cloneNode(true);
      header.querySelector('.menu-toggle').remove();
      header.querySelector('.brand').setAttribute('href', '/');
      for (const link of header.querySelectorAll('a[href^="#"]')) link.setAttribute('href', '/' + link.getAttribute('href'));
      document.querySelector('header').replaceWith(header);
      const footer = homeTemplate.querySelector('footer').cloneNode(true);
      for (const link of footer.querySelectorAll('a[href^="#"]')) link.setAttribute('href', '/' + link.getAttribute('href'));
      if (document.querySelector('footer')) document.querySelector('footer').replaceWith(footer);
      else document.body.append(footer);
      const main = document.querySelector('main');
      const hero = document.createElement('div');
      hero.className = 'document-hero';
      while (main.firstElementChild && !main.firstElementChild.matches('h2, section, .document-note, .document-toc, .button')) hero.append(main.firstElementChild);
      main.prepend(hero);
      main.classList.remove('section-wrap');
    }
    // 内容导航进入静态HTML，保证新页面能从首页和说明页被发现。
    const resources = document.createElement('nav');
    resources.className = 'resource-library section-wrap';
    resources.setAttribute('aria-label', '产品资料');
    const resourceHeading = document.createElement('h2');
    resourceHeading.textContent = '了解更多，让资料有处可寻。';
    resources.append(resourceHeading);
    for (const [path, label] of [['/about', '了解 WorkTwill'], ['/contract-workspace', '客户合同整理步骤'], ['/download', '下载、版本与支持'], ['/guide', '安装与使用说明'], ['/privacy', '隐私说明']]) {
      const link = document.createElement('a');
      link.href = path;
      link.textContent = label;
      resources.append(link);
    }
    if (page !== 'index') document.querySelector('footer')?.before(resources);
    document.documentElement.lang = locale;
    if (locale === 'en') {
      const walker = document.createTreeWalker(document.documentElement, NodeFilter.SHOW_TEXT);
      while (walker.nextNode()) walker.currentNode.textContent = translate(walker.currentNode.textContent);
      for (const element of document.querySelectorAll('*')) {
        for (const attribute of ['aria-label', 'placeholder', 'alt', 'content', 'value']) {
          if (element.hasAttribute(attribute)) element.setAttribute(attribute, translate(element.getAttribute(attribute)));
        }
      }
    }

    for (const anchor of document.querySelectorAll('a[href]')) {
      const href = anchor.getAttribute('href');
      if(locale==='en'&&href==='https://support.apple.com/zh-cn/102445')anchor.href='https://support.apple.com/en-us/102445';
      if (!href.startsWith('/') || href.startsWith('//')) continue;
      const [path, fragment] = href.split('#');
      const route = path.replace(/\.html$/, '');
      if (Object.values(pages).some((page) => page.route === route)) {
        anchor.setAttribute('href', localizedRoute(locale, route) + (fragment ? `#${fragment}` : ''));
      }
    }

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
    // 所有页面共享语言入口位置，手机端不藏入菜单。
    header.querySelector('.language-switch').remove();
    header.querySelector('.nav-actions').before(languages);

    for (const element of document.querySelectorAll('[data-updated]')) {
      element.setAttribute('datetime', meta.updated);
      element.textContent = meta.updated;
    }
    for (const element of document.querySelectorAll('[data-release]')) {
      const field = element.getAttribute('data-release');
      const value = field === 'stage' && locale === 'en' ? release.stage_en : field === 'bytes' ? `${release.bytes.toLocaleString(locale)} bytes (${(release.bytes/1048576).toFixed(1)} MiB)` : release[field];
      if (value === undefined) throw new Error(`未知发行字段：${field}`);
      element.textContent = value;
    }
    for (const element of document.querySelectorAll('[data-release-download]')) element.href = downloadUrl(release);
    for (const container of document.querySelectorAll('[data-platform-downloads]')) {
      for (const [target,names] of Object.entries(platformNames)) {
        if(target===release.platform)continue;
        const entry=release.platforms[target];
        const row=document.createElement('div');row.className='platform-download';
        const name=document.createElement('strong');name.textContent=locale==='en'?names.en:names.zh;row.append(name);
        if(entry){
          const link=document.createElement('a');link.className='button button-primary';link.href=downloadUrl(entry);link.setAttribute('download','');link.textContent=locale==='en'?`Download ${entry.version}`:`下载 ${entry.version}`;row.append(link);
          const size=document.createElement('small');size.textContent=`${(entry.bytes/1048576).toFixed(1)} MiB · SHA-256: ${entry.sha256}`;row.append(size);
        }else{const pending=document.createElement('span');pending.textContent=locale==='en'?'Installer not published yet':'安装包尚未发布';row.append(pending);}
        container.append(row);
      }
    }
    const intro=document.querySelector('[data-release-intro]');
    if(intro)intro.textContent=locale==='en'?'Choose a platform below. Published installers are available directly from this website without GitHub. These are preview builds; platform acceptance is tracked separately.':'从下方选择平台，已发布的安装包可直接下载，无需访问 GitHub。当前为预览版本，各平台验收状态单独记录。';
    if(page==='download'){
      document.querySelector('.release-panel h2').textContent=platformNames[release.platform][locale==='en'?'en':'zh'];
      setMeta(document,'description',locale==='en'?'Download WorkTwill installers directly from the official website. Check available Windows and Mac builds, release versions, checksums, installation steps, and app update support.':'从 WorkTwill 官网直接下载安装包，查看 Windows 和 Mac 的可用版本、校验值、安装方式与应用内更新支持范围。');
      const matrix=document.querySelector('h2 + dl.fact-list');
      if(matrix){
        matrix.children[0]?.remove();matrix.children[0]?.remove();
        for(const [target,names] of Object.entries(platformNames).reverse()){
          const row=document.createElement('div');const title=document.createElement('dt');title.textContent=locale==='en'?names.en:names.zh;
          const description=document.createElement('dd');description.textContent=release.platforms[target]?(locale==='en'?'A local candidate is available. Installer and update acceptance are tracked separately.':'已有本地候选包；安装与更新的实际验收单独记录。'):(locale==='en'?'Installer not published yet.':'安装包尚未发布。');row.append(title,description);matrix.prepend(row);
        }
      }
    }

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
        alternate.href = origin + localizedRoute(language === 'x-default' ? 'en' : language, meta.route);
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
        url: canonical, applicationCategory: 'BusinessApplication', operatingSystem: Object.keys(release.platforms).map(target=>platformNames[target].en).join(', '),
        softwareVersion: release.version, description, inLanguage: locales,
        sameAs: repository,
        downloadUrl: new URL(downloadUrl(release),origin).href, releaseNotes: origin + localizedRoute(locale, '/guide'),
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
      // 原型卡片只替换发行信息，不注入旧官网的下载或演示布局。
      for (const card of document.querySelectorAll('[data-home-download]')) {
        const entry = release.platforms[card.dataset.homeDownload];
        if (!entry) continue;
        let link = card;
        if (card.tagName !== 'A') {
          link = document.createElement('a');
          for (const attribute of card.attributes) if (!['type', 'style'].includes(attribute.name)) link.setAttribute(attribute.name, attribute.value);
          link.innerHTML = card.innerHTML;
          card.replaceWith(link);
        }
        link.classList.replace('coming', 'available');
        link.href = downloadUrl(entry);
        link.setAttribute('download', '');
        link.querySelector('small').textContent = `${locale === 'en' ? 'Preview' : '预览版'} · v${entry.version}`;
        link.title = `SHA-256: ${entry.sha256}`;
      }
    }
    const folder = new URL(`.${localizedRoute(locale, '/')}`, site);
    await mkdir(folder, { recursive: true });
    await writeFile(new URL(`${page}.html`, folder), dom.serialize());
    dom.window.close();
  }
}

const entries = Object.values(pages).filter((page) => page.route !== '/404').flatMap(({ route }) => locales.map((locale) => {
  const alternates = [...locales, 'x-default'].map((language) => `<xhtml:link rel="alternate" hreflang="${language}" href="${origin}${localizedRoute(language === 'x-default' ? 'en' : language, route)}"/>`).join('');
  const updated = Object.values(pages).find((page) => page.route === route).updated;
  return `<url><loc>${origin}${localizedRoute(locale, route)}</loc>${updated ? `<lastmod>${updated}</lastmod>` : ''}${alternates}</url>`;
}));
// 旧英文地址已公开，迁移后保留永久跳转，避免形成重复页面和失效链接。
const redirects = Object.values(pages).filter((page) => page.route !== '/404').map(({ route }) => `/en${route} ${localizedRoute('en', route)} 301`);
await writeFile(new URL('public/_redirects', site), redirects.join('\n') + '\n');
await writeFile(new URL('public/sitemap.xml', site), `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">\n${entries.join('\n')}\n</urlset>\n`);
const llms = `# WorkTwill\n\n> A local personal workspace for files and saved links. Resources are organized by reference; original files stay in place. File search and organization work offline without an account or AI setup.\n\nThe open-source repository is ${repository}. Consult the repository for source code, licensing, and contribution information. Current primary download: version ${release.version}, ${platformNames[release.platform].en}, ${release.stage_en}. Published platforms: ${Object.keys(release.platforms).map(target=>platformNames[target].en).join(", ")}. These are local candidates. OS code signing and notarization are deferred; update packages are independently signed. Production installation and online updates require separate acceptance. Search covers metadata, not document contents.\n\n## English\n${Object.values(pages).filter((page) => page.route !== '/404').map((page) => `- [${page.enTitle}](${origin}${localizedRoute('en', page.route)})`).join('\n')}\n\n## 简体中文\n${Object.values(pages).filter((page) => page.route !== '/404').map((page) => `- [${page.title}](${origin}${localizedRoute('zh-CN', page.route)})`).join('\n')}\n\n## Current release\n- [Download and verified support limits](${origin}${localizedRoute('en', '/download')})\n- SHA-256: ${release.sha256}\n\nThis file is a navigation aid, not a guarantee of AI search indexing, citations, or model training permission. Use the dated support page for release boundaries.\n`;
await writeFile(new URL('public/llms.txt', site), llms);
console.log(`已生成中英文${Object.keys(pages).length * 2}个静态页面、演示文案、多语言站点地图和llms导航。`);
