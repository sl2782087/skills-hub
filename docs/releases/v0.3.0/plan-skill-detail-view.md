# 技能详情页（Skill Detail View）— 实施计划（已完成）

## Context

当前已安装的技能以卡片列表展示，但无法查看技能的具体文件内容。用户希望点击技能卡片后能看到技能的所有文件，默认显示 SKILL.md，支持切换文件和返回列表。

### 设计原型

原型图位于 `docs/skillverse_v2_design.html` 的 Screen 5A–5D 部分。

### 交互流程

1. 在 My Skills 列表中，技能名称为可点击链接（hover 变蓝色）
2. 点击后整个内容区替换为详情视图（非模态框），Header 导航栏保持不变，My Skills tab 保持高亮
3. 详情视图顶部显示返回按钮、技能名称、描述、来源、更新时间、文件数
4. 下方为左右分栏：左侧文件树（260px），右侧文件内容（语法高亮 / Markdown 渲染）
5. 默认选中 SKILL.md（排首位），点击左侧其他文件切换内容
6. 文件夹默认折叠，点击展开/收起
7. 点击返回按钮回到列表视图

## 核心设计决策

### 视图切换而非模态框

采用扩展 `activeView` 状态增加 `'detail'` 视图的方式，与现有的 `'myskills'` / `'explore'` 视图切换机制一致。优势：
- 复用现有视图切换逻辑
- 详情视图可占满整个内容区域，空间充足
- detail 时 Header 仍高亮 My Skills tab，保持导航一致性

### 文件内容渲染策略（三层）

根据文件类型选择不同渲染方式：
1. **Markdown 文件**（`.md`/`.mdx`）：使用 `react-markdown` + `remark-gfm` + `remark-frontmatter` 渲染为 GitHub 风格 Markdown（标题、表格、代码块高亮、引用块等），YAML frontmatter 自动剥离不显示
2. **代码文件**（`.ts`/`.js`/`.py`/`.rs` 等 40+ 种语言）：使用 `react-syntax-highlighter`（Prism）语法高亮 + 行号，自动检测暗色/亮色主题切换 `oneDark`/`oneLight` 配色
3. **其他文件**：纯文本显示 + 行号

### 左侧文件树（自建，非第三方库）

将扁平路径构建为树形结构，文件夹可折叠/展开，默认折叠。排序：目录在前 → 文件在后，SKILL.md 排首位，其余按字母排序。

### 文件遍历复用 content_hash.rs 的过滤模式

`list_files` 使用与 `content_hash.rs` 相同的 walkdir + IGNORE_NAMES 过滤（排除 `.git`、`.DS_Store` 等），保持一致性。

---

## 步骤一：后端 — 新建 `src-tauri/src/core/skill_files.rs`

复用 `content_hash.rs` 的 walkdir + IGNORE_NAMES 过滤模式。

### 两个函数

**`list_files(central_path: &Path) -> Result<Vec<FileEntry>>`**
- 使用 walkdir 遍历目录，过滤 IGNORE_NAMES
- 返回 `Vec<FileEntry>`（相对路径 + 文件大小）
- SKILL.md 排在首位（排序时特殊处理）
- 其余文件按路径字母排序

**`read_file(central_path: &Path, relative_path: &str) -> Result<String>`**
- 路径穿越防护：禁止包含 `..`，canonicalize 后验证仍在 central_path 内
- 1MB 大小限制，超出返回友好错误信息
- 非 UTF-8 文件返回明确错误提示
- 读取并返回文件内容字符串

### FileEntry 结构体

```rust
pub struct FileEntry {
    pub path: String,  // 相对路径
    pub size: u64,     // 文件大小（字节）
}
```

### 模块导出

在 `src-tauri/src/core/mod.rs` 中添加 `pub mod skill_files;`。

---

## 步骤二：后端 — 新增 Tauri 命令

在 `src-tauri/src/commands/mod.rs` 中：

### DTO

```rust
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillFileEntry {
    pub path: String,
    pub size: u64,
}
```

### 命令

```rust
#[tauri::command]
pub async fn list_skill_files(central_path: String) -> Result<Vec<SkillFileEntry>, String>
// 调用 core::skill_files::list_files，转换为 DTO

#[tauri::command]
pub async fn read_skill_file(central_path: String, file_path: String) -> Result<String, String>
// 调用 core::skill_files::read_file
```

### 注册

在 `src-tauri/src/lib.rs` 的 `generate_handler!` 中添加：
- `commands::list_skill_files`
- `commands::read_skill_file`

---

## 步骤三：前端 — 类型定义

在 `src/components/skills/types.ts` 中新增：

```typescript
export type SkillFileEntry = {
  path: string
  size: number
}
```

---

## 步骤四：前端 — 新建 `src/components/skills/SkillDetailView.tsx`

### 依赖

- `react-syntax-highlighter`（Prism 版）— 代码高亮 + 行号，40+ 语言，oneLight/oneDark 主题
- `react-markdown` — Markdown 渲染
- `remark-gfm` — GitHub Flavored Markdown（表格、删除线、任务列表等）
- `remark-frontmatter` — 剥离 YAML frontmatter（SKILL.md 头部 metadata）

### Props

```typescript
type SkillDetailViewProps = {
  skill: ManagedSkill
  onBack: () => void
  invokeTauri: <T>(command: string, args?: Record<string, unknown>) => Promise<T>
  formatRelative: (ms: number | null | undefined) => string
  t: TFunction
}
```

### 内部状态

- `files: SkillFileEntry[]` — 文件列表
- `activeFile: string | null` — 当前选中的文件路径
- `fileContent: string` — 当前文件内容
- `loadingFiles: boolean` — 文件列表加载中
- `loadingContent: boolean` — 文件内容加载中
- `expanded: Set<string>` — 已展开的文件夹路径集合

### 内部子组件

- **`FileTreeNode`**（memo）— 递归渲染文件树节点，文件夹带 ChevronRight/ChevronDown + Folder/FolderOpen 图标
- **`FileContentRenderer`**（memo）— 根据文件类型选择 Markdown / SyntaxHighlighter / 纯文本渲染

### 行为

- `useEffect` 挂载时调用 `invoke('list_skill_files', { centralPath })` 获取文件列表
- 将扁平路径通过 `buildTree()` 构建为树形结构
- 文件夹默认折叠（`expanded` 初始为空 Set）
- 获取到文件列表后，默认选中第一个文件（应为 SKILL.md）
- `activeFile` 变化时调用 `invoke('read_skill_file', { centralPath, filePath })` 读取内容
- 自动检测暗色/亮色主题（读取 `data-theme` 属性），切换语法高亮配色
- 错误处理：通过 toast 显示错误信息

### 布局

```
┌──────────────────────────────────────────────────────┐
│ [← Back]                                              │
│ 技能名称                                               │
│ 描述                                                   │
│ 来源 · 更新时间 · N files                               │
├──────────────┬───────────────────────────────────────┤
│ FILES        │  file-path                      size   │
│ ▸ skills/    │┌─────────────────────────────────────┐│
│ ▾ examples/  ││  Markdown 渲染 / 语法高亮 + 行号     ││
│   basic.md   ││                                     ││
│   adv.md     ││                                     ││
│ SKILL.md ●   │└─────────────────────────────────────┘│
└──────────────┴───────────────────────────────────────┘
```

---

## 步骤五：前端 — 修改 `src/App.tsx`

### 状态变更

- `activeView` 类型扩展为 `'myskills' | 'explore' | 'detail'`
- 新增 `detailSkill: ManagedSkill | null` 状态

### 回调函数

```typescript
const handleOpenDetail = useCallback((skill: ManagedSkill) => {
  setDetailSkill(skill)
  setActiveView('detail')
}, [])

const handleBackToList = useCallback(() => {
  setDetailSkill(null)
  setActiveView('myskills')
}, [])
```

### 渲染

在 `skills-main` 区域增加 `activeView === 'detail'` 分支，渲染 `<SkillDetailView>`。

---

## 步骤六：前端 — 修改 `src/components/skills/SkillCard.tsx`

- 新增 `onOpenDetail: (skill: ManagedSkill) => void` prop
- `.skill-name` 改为 `<button>` 元素，添加 `onClick={() => onOpenDetail(skill)}`
- 添加 `clickable` CSS class（hover 变蓝色）

---

## 步骤七：前端 — 修改 `src/components/skills/SkillsList.tsx`

- 新增 `onOpenDetail` prop
- 透传到每个 `<SkillCard>` 组件

---

## 步骤八：前端 — 修改 `src/components/skills/Header.tsx`

- `activeView` 类型扩展为 `'myskills' | 'explore' | 'detail'`
- detail 视图时 My Skills tab 保持高亮状态（判断条件改为 `activeView === 'myskills' || activeView === 'detail'`）

---

## 步骤九：样式 — `src/App.css`

新增详情视图相关 CSS class：

**布局结构：**
- `.detail-view` — 整体容器（flex column, 填满空间）
- `.detail-header` — 顶部技能信息区
- `.detail-back-btn` — 返回按钮（hover 变蓝）
- `.detail-skill-name` — 技能名称（大号加粗）
- `.detail-desc` — 描述文字
- `.detail-meta` — 来源/时间/文件数元信息行
- `.detail-body` — 左右分栏容器（flex row）

**文件树侧栏（260px）：**
- `.detail-file-list` — 侧栏容器（bg-panel 背景）
- `.file-list-title` — "Files" 标题
- `.file-tree` — 树容器
- `.tree-item` — 树节点（通用，28px 最小高度）
- `.tree-dir` — 目录节点
- `.tree-file` — 文件节点（active 蓝色高亮）
- `.tree-chevron` — 折叠箭头
- `.tree-icon-folder` — 文件夹图标（蓝色）
- `.tree-icon-file` — 文件图标
- `.tree-name` — 名称（ellipsis 截断）
- `.tree-size` — 文件大小

**文件内容区：**
- `.detail-file-content` — 右侧内容区
- `.file-content-header` — 文件路径 + 大小（sticky top, bg-panel 背景）
- `.file-content-body` — 内容容器

**Markdown 渲染样式：**
- `.markdown-body` — Markdown 容器（max-width 860px，GitHub 风格排版）
- `.markdown-body h1/h2` — 标题带底部边框
- `.markdown-body pre` — 代码块（border + border-radius）
- `.markdown-body .md-inline-code` — 行内代码
- `.markdown-body table/th/td` — 表格样式
- `.markdown-body blockquote` — 引用块（蓝色左边框）

**通用：**
- `.skill-name.clickable` — 卡片中可点击的技能名称

暗色主题通过 CSS 变量自动适配，无需额外样式。

---

## 步骤十：i18n — `src/i18n/resources.ts`

新增翻译 key（`detail` 命名空间）：

| Key | EN | ZH |
|-----|----|----|
| `detail.back` | Back | 返回 |
| `detail.files` | Files | 文件 |
| `detail.noFiles` | No files found | 未找到文件 |
| `detail.loadingFiles` | Loading files... | 加载文件中... |
| `detail.loadingContent` | Loading file content... | 加载文件内容中... |
| `detail.readError` | Failed to read file | 读取文件失败 |
| `detail.fileCount` | {{count}} files | {{count}} 个文件 |

---

## 新增依赖

| 包名 | 用途 |
|------|------|
| `react-syntax-highlighter` | 代码语法高亮 + 行号（Prism 版，oneLight/oneDark 主题） |
| `@types/react-syntax-highlighter` | TypeScript 类型定义 |
| `react-markdown` | Markdown 渲染 |
| `remark-gfm` | GitHub Flavored Markdown 支持 |
| `remark-frontmatter` | YAML frontmatter 剥离 |

---

## 修改文件清单

| 文件 | 操作 | 说明 |
|------|------|------|
| `package.json` | 修改 | 新增 5 个依赖 |
| `src-tauri/src/core/skill_files.rs` | **新建** | list_files + read_file 核心逻辑 |
| `src-tauri/src/core/mod.rs` | 修改 | 导出 skill_files 模块 |
| `src-tauri/src/commands/mod.rs` | 修改 | 新增 2 个命令 + SkillFileEntry DTO |
| `src-tauri/src/lib.rs` | 修改 | 注册 list_skill_files、read_skill_file |
| `src/components/skills/types.ts` | 修改 | 新增 SkillFileEntry 类型 |
| `src/components/skills/SkillDetailView.tsx` | **新建** | 详情视图组件（文件树 + Markdown/高亮渲染） |
| `src/components/skills/SkillCard.tsx` | 修改 | 新增 onOpenDetail prop + 点击事件 |
| `src/components/skills/SkillsList.tsx` | 修改 | 透传 onOpenDetail prop |
| `src/components/skills/Header.tsx` | 修改 | activeView 类型扩展 |
| `src/App.tsx` | 修改 | 新增状态 + 渲染分支 + import SkillDetailView |
| `src/App.css` | 修改 | 新增文件树 + Markdown + 详情视图样式 |
| `src/i18n/resources.ts` | 修改 | 新增翻译 key（中英双语） |

---

## 验证

1. `npm run check` — 确保 lint + build + rust clippy/test 全部通过 ✅
2. `npm run tauri:dev` — 手动测试：
   - 点击技能名称进入详情视图
   - 默认显示 SKILL.md 内容（Markdown 渲染，frontmatter 已剥离）
   - 切换代码文件，语法高亮 + 行号正确显示
   - 文件树目录可折叠/展开，默认折叠
   - 点击返回按钮回到列表
   - Header 导航状态正确（detail 时 My Skills 高亮）
   - 暗色/亮色主题下显示正常
   - 加载状态正确显示
