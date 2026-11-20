# WorkTwill 开发约定

默认中文。产品行为以 PRD.md 为准，服务职责和验证边界以 ARCHITECTURE.md 为准；风险/调研记录不是功能完成证明。

- 正式前端在 `desktop/`，原生入口在 `src-tauri/`，领域与 SQLite 在 `crates/worktwill-core/`。根目录 `app.js/styles.css/index.html` 和 `prototype-b/` 是保留原型，不在正式运行时加载。
- Rust 是授权、资源 UUID、工作区关系和持久数据的事实源。React 只持有交互状态，不用 localStorage 保存第二套正式业务数据。
- `model.rs` 为跨端 DTO 来源；修改后运行 `pnpm types`。不要手改 `desktop/src/contracts.ts`；`pnpm types:check` 检测漂移。
- 依赖安装仅在本项目自己的 pnpm 工作区进行，不对上级用户目录安装或修改依赖。Node 依赖和 Rust 依赖均提交锁文件；不擅自升级。
- 测试以 `WORKTWILL_TEST_ROOT` 指定已确认的临时目录；测试仅在内部新建自己的 TempDir。当前会话已确认 `/private/tmp/worktwill-spikes-20261119`。不要把业务文件当测试样本，也不在目录已存在时清空覆盖。
- 完整本地检查：`pnpm check --fixtures /private/tmp/worktwill-spikes-20261119`。原生开发：`pnpm desktop`；未签名 macOS 应用构建：`pnpm tauri build --debug --bundles app`。跨平台和真实软件验收另记，不由单元测试代替。
- 新逻辑添加必要的简短中文注释，不作无关重构。关键测试、权限、性能或恢复要求变动要说明依据，不删除失败检查来完成交付。
- 当前物理身份不明的同路径替换需要人工确认；Bookmark 返回路径不等于身份连续。尚未验证的安全保存、Windows、发布签名不得写成已支持。
