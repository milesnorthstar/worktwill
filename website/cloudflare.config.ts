import { defineConfig } from 'cf/config';

// 官网只发布显式构建的静态目录，不接入桌面业务数据和原生文件能力。
export default defineConfig({
  accountId: '07452b400c3062d1761ed1158151a998',
  worker: {
    name: 'worktwill-website',
    compatibilityDate: '2026-11-20',
    workersDev: true,
    domains: ['worktwill.haoclub.com'],
    assets: { notFoundHandling: '404-page' },
    observability: { enabled: true, traces: { enabled: true } },
  },
});
