export const origin = 'https://worktwill.haoclub.com';
export const locales = ['zh-CN', 'en'];
// Bing站长页面签发的公开所有权标记，不是访问凭据。
export const bingVerification = 'A4A285974B4ECA51AFFD5CE1ABF790F0';
export const pages = {
  index: { route: '/', title: 'WorkTwill · 本地文件与网页整理工作台', enTitle: 'WorkTwill · Local workspace for files and links' },
  guide: { route: '/guide', title: '安装与使用 · WorkTwill', enTitle: 'Setup and usage guide · WorkTwill' },
  privacy: { route: '/privacy', title: '隐私说明 · WorkTwill', enTitle: 'Privacy · WorkTwill' },
  about: { route: '/about', title: 'WorkTwill 是什么？本地个人工作台与开源计划', enTitle: 'What is WorkTwill? Local workspace and open-source plans', updated: '2026-11-20' },
  'contract-workspace': { route: '/contract-workspace', title: '不移动原文件，如何整理客户合同资料 · WorkTwill', enTitle: 'Organize contract files without moving originals · WorkTwill', updated: '2026-11-20' },
  download: { route: '/download', title: '下载、版本与支持范围 · WorkTwill', enTitle: 'Download, release status, and platform support · WorkTwill', updated: '2026-11-20' },
  '404': { route: '/404', title: '页面未找到 · WorkTwill', enTitle: 'Page not found · WorkTwill' },
};
export const localizedRoute = (locale, route) => locale === 'en' ? `/en${route}` : route;
