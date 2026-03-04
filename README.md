# Reference Tool

基于 Rust + Tauri 的桌面文献引用管理工具。支持导入 `.bib`、按 key 批量引用、自动编号与区间压缩，并维护完整的已引用文献总表。

## 核心功能

- 导入 `.bib` 文献库，按 key 去重并覆盖。
- 左上输入多个 key 执行引用，返回如 `[1]-[3], [5]` 的压缩编号。
- 左下按首次引用顺序输出完整参考文献列表（只读、可复制）。
- 右侧展示所有已导入 key（可滚动）。
- 缺失 key 时事务失败：整次引用不落盘、不产生部分写入。

## 本地存储

- 应用状态保存在 Tauri AppData 目录下 `library_state.json`。
- 状态包含：
  - 已导入文献 `entries`
  - 已引用顺序 `citation_order`

## 开发环境

安装依赖：

```bash
npm install
```

桌面开发（推荐）：

```bash
npm run tauri dev
```

仅前端预览：

```bash
npm run dev
```

## 构建与验证

前端构建：

```bash
npm run build
```

Rust 编译检查：

```bash
cd src-tauri
cargo check
```

单元测试 + 集成测试：

```bash
cd src-tauri
cargo test
```

仅运行集成测试：

```bash
cd src-tauri
cargo test --test workflow_integration
```

调试打包（当前配置输出 deb/rpm）：

```bash
npm run tauri build -- --debug
```

## 使用流程

1. 点击右侧 `导入 .bib`，选择一个 bib 文件。
2. 导入成功后，右侧会刷新 key 列表，并显示新增/覆盖统计。
3. 在左上引用框输入多个 key（支持逗号、空格、换行）。
4. 点击 `引用`，返回框显示压缩后的编号串。
5. 左下自动维护已引用文献总表，可一键复制。

## 当前默认格式（可扩展）

默认输出近似为：

```text
Author. Title[J]. Journal, Year, Volume(Number): Pages. DOI: ...
```

后续如需新增样式，可在 `src-tauri/src/formatter.rs` 扩展 `OutputFormat` 与 formatter 实现。
