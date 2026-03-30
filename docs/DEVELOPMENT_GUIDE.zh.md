# Skillverse 开发与 AI 协作指南

> 面向本项目开发者与 AI 编程助手（Cursor / Claude Code / Codex 等）的统一文档。  
> 目标：进入项目后 10 分钟内建立整体认知，能快速定位代码并安全迭代。

## 1. 项目定位

Skillverse 是一个跨平台桌面应用（Tauri 2 + React 19），用于统一管理 Agent Skills，并将技能同步到多个 AI 工具目录，实现：

- Install once, sync everywhere
- 统一可视化管理：技能、来源、同步状态
- 安全导入与接管：不默认覆盖用户已有目录

## 2. 技术栈与运行环境

### 前端

- React 19 + TypeScript 5.9（strict）
- Vite 7 + Tailwind CSS 4
- i18next（中英双语）
- sonner（通知）

### 后端

- Rust 2021（Tauri 2）
- SQLite（rusqlite bundled）
- git2 + 系统 git CLI 回退策略
- reqwest（rustls-tls，blocking）

### 关键命令

```bash
npm install
npm run dev
npm run tauri:dev

npm run lint
npm run build
npm run rust:test
npm run check
```

说明：`npm run check` 会串行执行 lint/build/rust fmt/clippy/test，是提交前推荐的完整校验入口。

## 3. 仓库目录速览

```text
skillverse/
├── src/                     # React 前端
│   ├── App.tsx              # 状态中心与主业务编排
│   ├── App.css              # 全局组件样式
│   ├── index.css            # CSS 变量 / 主题 token
│   ├── components/
│   │   ├── Layout.tsx
│   │   └── skills/          # 核心业务模块（列表、卡片、弹窗）
│   └── i18n/
│       ├── index.ts
│       └── resources.ts     # 中英文案
├── src-tauri/               # Rust 后端
│   └── src/
│       ├── lib.rs           # Tauri 启动与 command 注册
│       ├── commands/mod.rs  # IPC 命令层（DTO + 错误格式）
│       └── core/            # 业务核心（存储/导入/同步/扫描）
├── docs/                    # 文档与发布计划
├── scripts/                 # 版本/打包辅助脚本
└── package.json             # 前端与整仓命令入口
```

## 4. 总体架构

### 4.1 分层

- 前端：负责 UI、状态管理、调用 `invoke`
- 命令层（Rust commands）：参数解析、线程隔离、错误包装
- 核心层（Rust core）：文件系统、Git、同步策略、数据库操作
- 数据层：SQLite + 本机文件系统（中心仓库 + 工具目录）

### 4.2 前后端通信

前端通过 `@tauri-apps/api/core` 的 `invoke('command_name', args)` 调用后端。  
耗时任务统一放到 `spawn_blocking`，避免阻塞 UI。

### 4.3 核心业务闭环

1. 导入 skill（local/git）
2. 写入中心仓库（默认 `~/.skillverse`）
3. 写入 DB（`skills` + `skill_targets`）
4. 同步到目标工具目录（symlink/junction/copy）
5. UI 刷新托管列表与目标状态

## 5. 关键模块与职责

### 前端核心

- `src/App.tsx`：全局状态中心（无状态管理库）
- `src/components/skills/`：技能域 UI（列表、筛选、弹窗、状态交互）
- `src/i18n/resources.ts`：所有用户可见文案（新增文案必须中英双语）

### 后端核心

- `src-tauri/src/core/skill_store.rs`：SQLite ORM（skills / skill_targets / settings / discovered_skills）
- `src-tauri/src/core/installer.rs`：本地与 Git 导入、更新、临时目录管理
- `src-tauri/src/core/sync_engine.rs`：同步引擎（symlink -> junction -> copy）
- `src-tauri/src/core/onboarding.rs`：扫描已安装工具技能、聚合冲突
- `src-tauri/src/core/tool_adapters/mod.rs`：工具注册表与安装检测
- `src-tauri/src/core/content_hash.rs`：目录内容指纹计算

## 6. 重要数据模型

### `skills`

- 托管技能主表：名称、来源、中心目录路径、哈希、更新时间等

### `skill_targets`

- 技能同步目标：工具、目标路径、模式（symlink/junction/copy）、状态

### `settings`

- key-value 配置（中心仓库路径、工具检测快照等）

### `discovered_skills`

- 扫描发现记录预留表（当前主流程多在运行时聚合）

## 7. 核心流程（开发必知）

### 7.1 导入（Local / Git）

- local：复制目录到中心仓库 + 入库
- git：clone 到缓存临时目录 -> 复制到中心仓库 -> 入库
- multi-skill 仓库：先列候选，再按 subpath 安装

### 7.2 同步

同步优先级：

1. symlink
2. Windows 下尝试 junction
3. 失败回退 copy

`overwrite` 默认 false，目标存在会返回 `TARGET_EXISTS|...`。

### 7.3 更新

- 按来源重建内容到 staging 目录
- swap 替换中心目录
- copy 模式目标会主动回灌更新
- symlink/junction 目标天然跟随中心目录变化

### 7.4 删除

- 先清理 DB 中记录的 target
- 再删除中心目录与 skill 记录
- 不做“全盘扫描删目录”以降低误删风险

## 8. 前端开发约定

- TypeScript strict，禁止 unused locals/params
- 组件使用 PascalCase
- modal 采用 `if (!open) return null`
- 样式集中在 `src/App.css`（非 CSS Modules）
- 新 UI 文案必须走 i18n key，不允许硬编码
- DTO 需与 Rust `commands/mod.rs` 保持同步

## 9. 后端开发约定

- 业务逻辑放 `core/`，`commands/` 保持轻量
- Tauri command 参数保持 camelCase（兼容前端调用）
- 错误使用 `anyhow::Context` 增强上下文
- 新增 core 模块要在 `core/mod.rs` 导出

## 10. 错误契约与前端分流

后端约定前缀：

- `MULTI_SKILLS|...`：多技能仓库需用户选择
- `TARGET_EXISTS|...`：目标已存在且未覆盖
- `TOOL_NOT_INSTALLED|...`：工具未安装

前端应基于前缀做交互分流，而非只显示原始错误文本。

## 11. 常见开发任务：改哪里

### 新增一个后端 command

1. 在 `src-tauri/src/commands/mod.rs` 定义函数与 DTO
2. 在 `src-tauri/src/lib.rs` 的 `generate_handler!` 注册
3. 前端通过 `invoke` 调用并处理错误分支

### 新增一个工具适配器

1. 在 `tool_adapters/mod.rs` 增加 `ToolId` 与 adapter 实例
2. 定义 detect 目录 + skills 目录
3. 前端工具展示/映射处补充文案与显示名（如有）

### 新增一个 UI 文案

1. `src/i18n/resources.ts` 添加 en/zh
2. 组件里使用 `t('key')`
3. 避免直接写字符串

### 新增导入或同步逻辑

优先改 `core/installer.rs` 与 `core/sync_engine.rs`，然后同步更新 command DTO 与前端调用链。

## 12. AI 辅助开发实战建议

### 12.1 推荐工作流

1. 先让 AI 输出“改动文件清单 + 风险点”
2. 再执行最小范围实现（不要让 AI 顺手重构无关代码）
3. 完成后强制跑 `npm run lint` + `npm run build`
4. 涉及 Rust 再跑 `npm run rust:test`（或直接 `npm run check`）

### 12.2 给 AI 的上下文最小集

每次提需求时，建议附带：

- 目标功能与验收标准
- 涉及文件路径（如 `src/App.tsx`, `src-tauri/src/core/installer.rs`）
- 是否允许改动 UI、DTO、数据库结构
- 是否需要兼容历史数据

### 12.3 高质量提示词模板

```text
你在修改 Skillverse（Tauri + React + Rust）项目。
请按以下要求实现：
1) 先给出改动方案与涉及文件
2) 仅修改必要文件，不做无关重构
3) 前端文案必须接入 i18n（en + zh）
4) 若新增/变更 command，确保 lib.rs 完成注册
5) 完成后执行 npm run lint 与 npm run build，并反馈结果
```

### 12.4 常见坑位提醒

- 只改前端不改 command 注册，导致 invoke 找不到命令
- 新增文案未补中文或英文，造成 i18n key fallback
- DTO 前后端字段名不一致（尤其 camelCase / snake_case）
- 忽略 `overwrite=false` 默认策略，误判同步失败为 bug

## 13. 验证与发布前检查

最少执行：

```bash
npm run lint
npm run build
```

建议执行（完整）：

```bash
npm run check
```

额外建议：

- 手动验证一次 Local 导入 + Git 导入 + 同步 + 更新 + 删除闭环
- 若改 UI，补截图或录屏，便于 review

## 14. 推荐阅读顺序

1. `README.md` / `docs/README.zh.md`（产品能力）
2. `docs/releases/v0.1-v0.2/system-design.zh.md`（系统设计）
3. `src/App.tsx`（前端主流程）
4. `src-tauri/src/commands/mod.rs`（接口层）
5. `src-tauri/src/core/installer.rs` + `sync_engine.rs`（核心逻辑）

---

如果你准备开始一个具体需求，建议先写一版“影响面清单”（UI / command / core / i18n / tests），再动手改代码，可显著降低漏改概率。
