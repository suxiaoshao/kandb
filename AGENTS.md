# AGENTS.md

## 项目与实现约束

- kanDB 是基于 GPUI 的桌面数据库查看与管理客户端，处于早期 bootstrap 阶段；入口为 `crates/kandb/src/main.rs`，包与技术基线以 Cargo 配置为准。未实现的产品方向须标注为 roadmap 或 planned work。
- 在当前目标内选择最佳设计，直接修复建模、状态流、抽象或生命周期的根因，避免临时绕过与层层兜底。复用已有类型、日志和公共能力；按实际职责决定模块边界，不为假设的未来复杂度提前拆分。
- Rust 模块使用 `{module}.rs`，禁止新增 `mod.rs`；新增依赖使用完整版本号。文档与配置使用 UTF-8、LF。
- 当前改动影响 README、LICENSE 或公开元数据时，同步对应表述并保持仓库主页展示一致。

## UI 与技能入口

- 优先使用简单、可组合的 GPUI 高层 API，确有缺口时再使用 `Element`。组件职责清晰，布局、状态、数据访问与副作用保持边界；样式复用主题能力，组件 API 注重命名稳定和可组合性。
- 用版式、对齐、间距、字号、对比度和有目的的动效建立层级，避免无意义卡片、装饰渐变和多重强调色。
- 实现前按任务读取 `.agents/skills/` 中对应技能：

| 涉及内容 | skill |
| --- | --- |
| 组件风格、命名、API | `gpui-style-guide` |
| 布局、样式、主题 | `gpui-layout-and-style` |
| 低层 Element | `gpui-element` |
| 实体与数据流 | `gpui-entity` |
| 异步与后台工作 | `gpui-async` |
| 动作与事件 | `gpui-action`、`gpui-event` |
| 焦点与键盘 | `gpui-focus-handle` |
| 上下文与全局状态 | `gpui-context`、`gpui-global` |
| GPUI 测试 | `gpui-test` |

## GitHub 与验证

- Issue、PR 等内容遵循 `.github/` 中适用的模板、workflow 和约定；PR 描述覆盖当前分支相对远程最新 `main` 的整体差异。
- 行为变更验证关键不变量，优先复用已有覆盖。按影响选择构建、测试或严格 Clippy，并保留适用 CI 与 hooks。
- 文档与指令改动仅检查相关结构、链接和差异；汇报实际验证命令，以及未完成的适用检查和原因。
