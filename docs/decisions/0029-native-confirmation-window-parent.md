# 0029：原生确认绑定发起窗口

日期：2026-11-20。状态：已实现，检查与原生复验分别记录于032。

## 事实与路径

032实际点击恢复备份后，React处于busy，WorkTwill原生AX和窗口截图均未显示确认框，Esc未释放busy。未接受确认、未选备份文件，未执行恢复。应用经Computer-Use正常退出，CLI只读进程核对为0。

当前锁定的`tauri-plugin-dialog 2.8.4`经`rfd 0.17.2`显示确认。直接核查本机依赖源码：未设置parent时，macOS异步消息进入`utils::async_pop_dialog -> CFUserNotificationDisplayAlert`独立系统提醒路径；有parent时使用`ModalFuture -> beginSheetModalForWindow`。原代码四个确认均无parent。该路径解释确认不属于应用窗口；不能据此声称系统提醒在所有显示器上都不存在。

状态归属保持：React的run等待Rust命令；Rust先确认再取选择器结果，恢复仍由Store暂存校验/迁移/备份/替换与撤权。不能用前端自行解busy或跳过确认掩盖等待。

## 决定

- `restore`、`pick_relink`、`import_drop`接收Tauri注入的调用`WebviewWindow`；前端invoke参数不变，不接受JS传入的窗口或新授权数据。
- 四个消息builder（恢复、重新指向、合并、所选文件授权）统一`.parent(&window)`。确认文案、按钮含义和批准后的领域流程保持不变。
- 不升级依赖，不变DTO/schema，不建立另一套React确认/权限事实源，不改文件选择器、目录授权或源文件。
- 取消必须返回原false/未授权分支并释放busy；是否真正接受恢复属于另一次有影响的动作，不能把取消复验写成完整恢复通过。

主要代价是确认与发起窗口的生命周期绑定；这符合本项目当前单窗口运行。未来多窗口仍由调用窗口归属，不使用全局猜测窗口。

## 验证

完整检查207 Rust/137 UI通过，覆盖既有授权、关系、恢复、确认双语与失效审阅回归，并验证注入窗口参数可编译；没有新增仅断言builder调用形式的测试。修前原生等待、修后确认可见/取消/释放busy及最终数据边界以[032记录](../validation/2026-11-20-native-acceptance-032.json)和验收段为准，构建本身不算原生通过。
