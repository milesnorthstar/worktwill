// 演示内容的唯一来源；构建时与页面共用英文词典生成两种语言。
export default {
  resources: [
    { name: '春日品牌企划.docx', location: '文档 / 品牌企划', type: 'DOCX', kind: 'file', icon: 'word', letter: 'W', spaces: ['spring', 'inspiration'], favorite: true },
    { name: '项目预算.xlsx', location: '文档 / 项目资料', type: 'XLSX', kind: 'file', icon: 'sheet', letter: 'X', spaces: ['spring'], favorite: false },
    { name: '品牌视觉参考.pdf', location: '下载 / 参考资料', type: 'PDF', kind: 'file', icon: 'pdf', letter: 'P', spaces: ['spring'], favorite: true },
    { name: '设计灵感 · Are.na', location: 'are.na', type: '网页', kind: 'web', icon: 'web', letter: '↗', spaces: ['spring', 'inspiration'], favorite: false },
    { name: '新同事入职清单.xlsx', location: '文档 / 人事资料', type: 'XLSX', kind: 'file', icon: 'sheet', letter: 'X', spaces: ['onboarding'], favorite: true },
    { name: '入职资料说明.docx', location: '文档 / 人事资料', type: 'DOCX', kind: 'file', icon: 'word', letter: 'W', spaces: ['onboarding'], favorite: false },
  ],
  workspaceNames: { spring: '春日品牌企划', onboarding: '新同事入职', inspiration: '灵感与参考' },
  views: {
    home: ['工作的一切，在这里。', '找回资料，接着上次的思路继续。', '个人工作台'],
    workspaces: ['给每一件工作，一个空间。', '选择一个工作区，看看相关的文件与网页。', '工作区'],
    activity: ['让工作的来路，清晰可见。', '按时间回顾修改、首次发现与应用内打开。', '动态'],
    files: ['所有资料，一个入口。', '查找授权位置中的文件与已保存的网页。', '所有文件'],
    favorites: ['常用的，留在手边。', '为下一次需要，少找一步。', '收藏'],
  },
  labels: {
    searchTitle: '找回你需要的资料。',
    searchDescription: '从示例资料的名称和位置中查找。',
    workspaceDescription: '这一件事需要的资料，都在这里。',
    workspace: '工作区',
    results: '搜索结果 · {count} 项',
    workspaceResources: '工作区资料',
    favorites: '收藏的资料',
    recent: '最近的资料',
    empty: '没有匹配的示例资料，试试「企划」「入职」或「参考」。',
  },
};
