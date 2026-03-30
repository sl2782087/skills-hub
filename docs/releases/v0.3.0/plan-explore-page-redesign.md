# 需求：Explore 页面独立化 + My Skills 列表优化

## 背景

当前"添加 Skill"的交互流程存在问题：探索、本地添加、Git 添加三个功能挤在一个弹窗的三个 Tab 里。用户在探索 Tab 点击一个 skill 后，只是把 URL 填入 Git Tab 并跳转，还需手动确认并点击安装，流程冗长（5 步）。

## 设计稿

`docs/skillverse_v2_design.html` — 包含 4 个屏幕的完整交互设计。

## 改动概览

### 一、导航结构调整

**现状**：单页面 + 弹窗内 3 个 Tab（探索/本地/Git）
**目标**：顶部导航 2 个页面级 Tab + 弹窗仅保留手动添加

- App Header 内新增 **My Skills** / **Explore** 两个导航 Tab
- 点击切换页面视图（非路由，纯前端状态切换）
- 默认展示 My Skills 页

### 二、Explore 页（新页面）

将探索功能从弹窗提升为独立页面，作为"获取新 Skill 的唯一入口"。

#### 布局
- 顶部：搜索栏 + **Manual** 按钮（触发手动添加弹窗）
- 搜索栏下方：提示文字 "Data from clawhub.ai · Have a Git URL or local path? Click Manual to add directly"
- 内容区：双列卡片网格

#### Explore 卡片
每张卡片展示：
- Skill 名称
- 作者/来源（repo 路径）
- 描述（最多 2 行截断）
- 下载量 / Stars
- 兼容工具小标签（折叠显示，如 `Cursor` `Claude` `+5`）
- **Install 按钮**（右上角，一键安装）
- 已安装的 Skill 显示为绿色 **Installed** 状态（禁用按钮）

#### 一键安装流程
- 点击 Install → 自动使用所有已检测到的工具作为同步目标 → 安装并同步
- 安装成功后底部 toast 提示："xxx installed and synced to N tools"
- 安装完成后按钮状态变为 Installed

#### 搜索态
- 输入 ≥ 2 字符触发搜索
- 结果分为 "Featured Matches" 和 "Online Results" 两个区域
- 搜索关键词高亮

### 三、Manual Add 弹窗（精简）

- 从 Explore 页的 Manual 按钮触发
- **移除探索 Tab**，仅保留 Local Directory / Git Repository 两个 Tab
- 其余逻辑不变（工具选择器、安装流程）

### 四、My Skills 页（列表优化）

#### 移除
- 移除 Add 按钮（添加功能统一收口到 Explore 页）

#### 卡片增加 description
- 每张卡片新增描述文本，显示在名称下方，最多 2 行截断
- 信息层次：名称 → 描述 → 来源+时间 → 工具徽章

#### 工具徽章优化
- **只显示已同步的工具**（绿色带圆点），不再显示未同步的灰色徽章
- 超过 5 个折叠为 `+N more`
- 减少视觉噪音，让每张卡片高度一致

## 后端改动

### 新增 description 字段

当前 `ManagedSkillDto` 没有 description 字段，需要：

1. 从 Skill 目录的 `SKILL.md` frontmatter 中解析 `description` 字段
2. 在安装时提取并存入数据库（`skills` 表新增 `description` 列）
3. `ManagedSkillDto` 新增 `description: Option<String>` 字段返回给前端

### 涉及文件
- `src-tauri/src/core/skill_store.rs` — 表结构迁移，新增 description 列
- `src-tauri/src/core/installer.rs` — 安装时解析 SKILL.md 提取 description
- `src-tauri/src/commands/mod.rs` — DTO 新增字段

## 前端改动

### 涉及文件
- `src/App.tsx` — 新增页面级 Tab 状态，拆分 Explore 视图逻辑
- `src/App.css` — Explore 页样式、卡片优化样式
- `src/components/skills/types.ts` — DTO 同步新增 description 字段
- `src/components/skills/modals/AddSkillModal.tsx` — 移除探索 Tab，仅保留 Local/Git
- `src/i18n/resources.ts` — 新增/调整翻译 key（My Skills / Explore 导航等）

### 可能新增文件
- `src/components/skills/ExplorePage.tsx` — Explore 页面组件
- `src/components/skills/ExploreCard.tsx` — Explore 卡片组件

## 实施顺序建议

1. **后端**：description 字段（表迁移 + SKILL.md 解析 + DTO）
2. **前端**：导航 Tab 切换 + My Skills 列表优化（description 展示 + 工具徽章折叠）
3. **前端**：Explore 页面（搜索 + 卡片网格 + 一键安装）
4. **前端**：Manual Add 弹窗精简（移除探索 Tab，改为从 Explore 页触发）
5. **联调 & 测试**
