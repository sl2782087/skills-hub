# Skillverse UI 改版设计文档

**日期**：2026-03-30  
**范围**：全页面视觉与交互改版（功能不变）  
**状态**：已确认，待实现

---

## 1. 背景与目标

当前 Skillverse（原 Skillverse）UI 采用顶部水平导航 + 全宽内容区的传统布局，视觉层次较平，缺乏统计概览，扩展空间有限。

本次改版目标：
- 引入**左侧图标导航栏**，为后续功能扩展预留空间
- 增加**顶部统计概览区**，用户一眼掌握全局状态
- 增强**每技能的同步操作可及性**，展开即可操作，减少认知跳转
- 整体视觉升级，保持深色主题，强化空间层次与交互质感

---

## 2. 布局架构

```
┌──────────────────────────────────────────────────────┐
│  Rail(52px) │  Main Content Area (flex:1)             │
│  ──────── │  ─────────────────────────────────────  │
│  Logo      │  [Page Header]                          │
│  My Skills │    Stats Row (4 cards)                  │
│  Explore   │  [Filter Bar]                           │
│  ────────  │    Search | All/Git/Local | Sort | +Add │
│  Settings  │  [Skills List]                          │
│  (bottom)  │    SkillCard (collapsed/expanded)       │
└──────────────────────────────────────────────────────┘
```

整体高度充满视口，左侧 Rail 固定，Main Content 内部滚动。

---

## 3. 左侧图标导航栏（Rail）

**宽度**：52px，固定，不折叠。

| 位置 | 图标 | 对应页面 | Tooltip |
|------|------|----------|---------|
| 顶部 | Logo（Skillverse 标志）| — | — |
| 上方 | Grid 图标 | My Skills | "My Skills" |
| 上方 | Search 图标 | Explore | "Explore" |
| 分割线 | — | — | — |
| 底部 | Languages 图标 | — | 语言切换（EN/中）|
| 底部 | Settings 图标 | Settings | "Settings" |

- 激活状态：`rgba(99,102,241,0.18)` 背景 + indigo 文字色
- Hover 时右侧浮出 tooltip（绝对定位，不影响布局）
- 样式：`border-right: 1px solid #1a2540`，背景 `#070d1a`（比主内容区略深）

---

## 4. My Skills 页

### 4.1 统计概览区（Stats Row）

4 个等宽卡片，位于页面标题下方，筛选栏上方：

| 卡片 | 指标 | 子文案 |
|------|------|--------|
| Total Skills | 技能总数（数字大） | ↑ N this week |
| Tools Active | 已激活工具数 | of 47 supported |
| Synced | 已完全同步数 | N partial |
| Last Updated | 最近更新技能名 | 相对时间 |

- 第一张卡带 accent 样式（`rgba(99,102,241,0.06)` 背景 + indigo 边框）
- 其余卡风格统一，`#0f172a` 背景
- 卡片 hover 时边框加亮

### 4.2 筛选栏（Filter Bar）

从左到右：
1. **搜索框**（flex:1）：全宽占满，圆角 8px，`#0f172a` 背景
2. **快捷过滤**：`All(N)` / `Git` / `Local`，胶囊样式，active 时 indigo 配色
3. **排序下拉**：`Sort ▾`（保留现有逻辑）
4. **Add Skill 按钮**：indigo 实色，右侧固定

### 4.3 技能列表

- 按来源分 Section（Git Skills / Local Skills），Section 标题带延伸分割线
- 每条 SkillCard 包含：
  - 左：技能缩写图标（2字母，彩色渐变）
  - 中：技能名 + 来源 tag（git/local） + 相对时间
  - 右：工具头像缩略（最多 3 个 + "+N"）+ 同步状态徽章（synced/partial）+ 展开箭头

#### 展开同步控制面板

点击卡片标题行 → 展开 sync panel（面板在卡片内部，非 modal）：

- **工具 toggle 网格**（4列，响应式）：每个 toggle 显示工具名 + 状态点，点击即切换
  - 已启用：indigo 底色 + 亮色点
  - 未启用：暗色底色 + 暗色点
- **操作按钮行**：
  - `↻ Sync Now`（indigo 实色）
  - `⬆ Update`（indigo 轮廓）
  - `Remove`（danger 轮廓，靠右）

---

## 5. Explore 页

保留原有搜索 + 精选网格 + 在线结果结构，视觉升级：

- 顶部搜索区（explore-hero）沿用渐变背景，搜索框与手动添加按钮样式与新 FilterBar 保持一致
- 精选卡片（explore-card）调整圆角、内间距、hover 阴影，与 SkillCard 视觉语言统一
- 滚动区延续已有的细滚动条美化

---

## 6. Settings 页

结构不变，视觉升级：
- Section 标题样式与新 FilterBar section-label 保持统一
- 输入框、按钮使用新设计系统的 radius/color token
- 背景统一为 `#0f172a` 卡片

---

## 7. 色彩与 Token 调整

现有 CSS 变量体系保持不变，新增/调整以下使用规范：

| 用途 | Token | 暗色值 | 亮色值 |
|------|-------|--------|--------|
| Rail 背景 | `--bg-rail` | `#070d1a` | `#f1f5f9` |
| Rail 边框 | `--border-rail` | `#1a2540` | `#e2e8f0` |
| Accent 软背景 | `--accent-soft-bg` | `rgba(99,102,241,0.06)` | `rgba(99,102,241,0.08)` |
| 激活导航 | inline（非 token）| `rgba(99,102,241,0.18)` | `rgba(99,102,241,0.14)` |

*已有 token（`--bg-panel`、`--border-subtle` 等）不变，`--border-subtle` 不复用于 Rail，避免语义重叠。*

字体、主 accent 颜色、圆角、阴影变量均保持不变。

---

## 8. 动画与交互规范

| 交互 | 动画 | 时长 |
|------|------|------|
| 导航切换（Rail 点击）| 无（即时）| — |
| SkillCard 标题行点击 | 展开/收起 sync panel（display toggle）| — |
| SkillCard 详情图标点击 | 打开原有详情 modal（`onOpenDetail`，行为不变）| — |
| 工具 toggle 点击 | border/background 渐变 | 0.15s ease |
| 统计卡 hover | border-color 渐变 | 0.15s |
| 按钮 hover | background 渐变 | 0.15s |

*注：SkillCard 展开可后续升级为 max-height 动画，本次暂用 display 切换保持实现简单。*

---

## 9. 文件改动范围

> **注意**：`src/components/Layout.tsx` 目前为未引用的死代码，不纳入改动。  
> **真实布局入口**：`App.tsx` 中的 `.skills-app`，改版在此处落地。

| 文件 | 改动类型 |
|------|----------|
| `src/App.tsx` | **布局主文件**：重组为 `Rail + main` 水平 flex，传递新 filter/activePage 状态 |
| `src/components/skills/Header.tsx` | 重构为左侧 `Rail` 组件（含 Logo、导航图标、语言切换、Settings 入口） |
| `src/components/skills/SkillCard.tsx` | 新增展开状态 + sync panel；保留详情入口（通过卡片右上角图标，不影响点击标题行的展开） |
| `src/components/skills/SkillsList.tsx` | 增加 Section 分组（Git/Local）+ 顶部 Stats Row 组件 |
| `src/components/skills/FilterBar.tsx` | 样式升级（All/Git/Local 胶囊）；**保留刷新按钮**（迁移至 FilterBar 右端或保持原位） |
| `src/components/skills/ExplorePage.tsx` | 视觉升级：卡片样式与新设计语言保持统一 |
| `src/App.css` | 全面升级对应 class 样式 |
| `src/index.css` | 补充新 CSS 变量（`--bg-rail`、`--border-rail` 等，深色/浅色双套） |
| `src/i18n/resources.ts` | 补充新 key（统计标签、筛选标签、Rail tooltip 等） |

---

## 10. 不在本次范围内

- Explore 页的搜索逻辑（不变）
- Rust 后端代码（不变）
- 路由与 IPC 逻辑（不变）
- CHANGELOG、版本号（不变）
