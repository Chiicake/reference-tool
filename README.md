# Reference Tool

一个基于 `Rust + Tauri + React` 的桌面文献引用管理工具，面向“按 BibTeX key 快速插入文献编号”的场景。

它解决的核心问题是：

- 维护本地文献库（`.bib` 导入）；
- 按 key 快速引用并生成编号；
- 自动维护“已有引用文献总表”；
- 支持在段落中直接替换 `\cite{...}` 命令。

## 主要功能

### 1) bib 导入与去重覆盖

- 支持导入 `.bib` 文件。
- 以 `key` 为唯一标识。
- 导入策略：
  - 新 key：新增；
  - 已存在 key：覆盖更新；
- 导入完成后返回统计：`total / imported / new / overwritten / failed`。

### 2) 两种引用输入模式

- key 列表模式：
  - 输入示例：`10495806,10648348,10980318`。
  - 输出示例：`[1]-[3]`。
- 段落模式（LaTeX cite 替换）：
  - 输入示例：
    `...增长\cite{8016573}...普及\cite{9221208,6425066}...`
  - 输出示例：
    `...增长[1]...普及[2],[3]...`
  - 仅替换 `\cite...{}` 片段，其他正文原样保留。

### 3) 编号与压缩规则

- 已引用 key：复用原编号。
- 新 key：分配新编号。
- 区间压缩：
  - `[1][2][3] -> [1]-[3]`
  - `[1][2] -> [1],[2]`（两个连续编号不使用短横线）
  - `[1][2][3][5] -> [1]-[3], [5]`

### 4) 下一个序号可手动设置

- 可以直接设置“下一个引用序号”（例如设置为 `16`）。
- 该设置对**后续首次引用的新 key**生效，不会改动已经分配过的历史编号。
- 输入留空时，按“当前最大编号 + 1”自动计算。

### 5) 清空操作

- 清空数据库：删除所有导入文献，并清空引用状态。
- 清空已有引用：仅清空引用状态，编号重置为从 `1` 重新计数。

## 界面说明

- 左上：引用操作区
  - 引用框
  - 引用返回框（只读）
  - 设置下一个序号
  - 复制返回 / 引用按钮
- 左下：已有引用文献（只读，可滚动）
  - 复制按钮
  - 清空引用按钮
- 右侧：已导入 key 列表（可滚动）
  - 导入 `.bib`
  - 清空数据库

## 事务与错误处理

- 引用时采用事务语义：
  - 如果本次输入中存在任意缺失 key，则整次失败，不会部分写入。
- 段落模式同样遵循事务语义。
- 不合法文件、空 bib、解析异常会给出明确错误提示。

## 本地存储

- 状态文件位置：Tauri `AppData` 目录下 `library_state.json`。
- 关键数据：
  - `entries`: 已导入文献字典（key -> entry）
  - `citation_order`: 已引用 key 的顺序列表
  - `citation_index_by_key`: key 到分配编号的映射
  - `next_citation_index`: 下一个新引用将使用的编号

## 项目结构（核心）

- `src-tauri/src/bib_parser.rs`: BibTeX 解析
- `src-tauri/src/formatter.rs`: 参考文献文本格式化
- `src-tauri/src/citation_engine.rs`: key 解析、cite 命令提取、编号压缩
- `src-tauri/src/state.rs`: 核心状态机与业务流程
- `src-tauri/src/commands.rs`: Tauri 命令接口
- `src/App.tsx`: 前端交互逻辑

## 快速开始

安装依赖：

```bash
npm install
```

启动桌面开发环境：

```bash
npm run tauri dev
```

仅启动前端（无 Tauri 后端能力）：

```bash
npm run dev
```

## 构建与测试

前端构建：

```bash
npm run build
```

Rust 编译检查：

```bash
cd src-tauri
cargo check
```

运行全部测试（单元 + 集成）：

```bash
cd src-tauri
cargo test
```

仅运行集成测试：

```bash
cd src-tauri
cargo test --test workflow_integration
```

调试打包（deb/rpm）：

```bash
npm run tauri build -- --debug
```

## 当前默认参考文献格式

```text
Author. Title[J]. Journal, Year, Volume(Number): Pages. DOI: ...
```

如需新增输出风格，可在 `src-tauri/src/formatter.rs` 扩展 `OutputFormat` 与 formatter 实现。
