export const origin = 'https://worktwill.haoclub.com';
export const locales = ['en', 'zh-CN'];
export const repository = 'https://github.com/milesnorthstar/worktwill';
// Bing站长页面签发的公开所有权标记，不是访问凭据。
export const bingVerification = 'A4A285974B4ECA51AFFD5CE1ABF790F0';
export const pages = {
  index: { route: '/', title: 'WorkTwill · 本地文件与网页整理工作台', enTitle: 'WorkTwill · Local workspace for files and links', updated: '2026-11-23' },
  guide: { route: '/guide', title: '安装与使用 · WorkTwill', enTitle: 'Setup and usage guide · WorkTwill', updated: '2026-11-23' },
  privacy: { route: '/privacy', title: '隐私说明 · WorkTwill', enTitle: 'Privacy · WorkTwill' },
  about: { route: '/about', title: 'WorkTwill 是什么？本地文件与网页工作台', enTitle: 'What is WorkTwill? A local workspace for files and links', updated: '2026-11-22' },
  'contract-workspace': { route: '/contract-workspace', title: '不移动原文件，如何整理客户合同资料 · WorkTwill', enTitle: 'Organize contract files without moving originals · WorkTwill', updated: '2026-11-20' },
  download: { route: '/download', title: '下载、版本与支持范围 · WorkTwill', enTitle: 'Download, release status, and platform support · WorkTwill', updated: '2026-11-23' },
  '404': { route: '/404', title: '页面未找到 · WorkTwill', enTitle: 'Page not found · WorkTwill' },
};
// 英文是默认入口，中文使用独立前缀，所有页面与 SEO 共用此映射。
export const localizedRoute = (locale, route) => locale === 'zh-CN' ? `/zh${route}` : route;
