[English](README.md) | [简体中文](README.zh-CN.md)

# gwtm

`gwtm` 是一个使用 Rust 编写的 Git worktree 管理工具，面向本地多项目工作流。

## 功能说明

- 扫描配置好的项目根目录，并识别其一级子目录中的 Git 仓库
- 创建、打开、列出和删除 Git worktree
- 提供全屏 TUI，用于首次初始化和后续重新配置
- 支持项目、分支、worktree 的可搜索选择列表
- 会检测本机可用的 IDE 启动方式，并让你选择默认用哪个打开 worktree

## 项目结构

- `src/main.rs`：程序入口和主流程逻辑
- `src/tui.rs`：全屏 TUI 组件与交互状态
- `Cargo.toml`：Rust 包清单
- `gwtm`：轻量启动脚本，优先运行可用的本地二进制，否则回退到 `cargo run`
- `RELEASING.md`：维护者发版 SOP

## 运行要求

- Rust toolchain
- Git
- macOS 下可选：`osascript`，用于调用系统文件夹选择器
- 可选：`PATH` 中存在 `rustrover` 命令

## 运行方式

开发模式：

```bash
./gwtm
```

或直接运行：

```bash
cargo run
```

发布构建：

```bash
cargo build --release
./target/release/gwtm
```

## Homebrew

这个仓库已经为通过个人 Homebrew tap 发布做好准备。

与 Homebrew 发布相关的文件：

- `packaging/homebrew-tap/Formula/gwtm.rb`
- `packaging/homebrew-tap/README.md`
- `RELEASING.md`
- `scripts/update-homebrew-formula.sh`

推荐发版流程：

```bash
git tag -a v<version> -m "v<version>"
git push origin main
git push origin v<version>
./scripts/update-homebrew-formula.sh <version>
```

然后把生成的 formula 复制到：

```text
life2you/homebrew-tap
```

发布 tap 仓库后，用户可以这样安装：

```bash
brew install life2you/tap/gwtm
```

## 首次启动

首次启动时，`gwtm` 会要求配置：

1. `projects_root_dir`：项目根目录，其一级子目录应为 Git 仓库
2. `worktrees_root_dir`：新建 worktree 的存放目录
3. 默认用于打开 worktree 的 IDE 或启动器

配置流程和主菜单使用同一套全屏 TUI。在 macOS 下，光标位于路径输入框时可按 `f` 打开系统文件夹选择器。路径确认后，`gwtm` 会自动检测本机可用的 IDE 启动命令和已安装应用，并让你明确选择一个默认打开方式；你也可以先跳过，等第一次打开 worktree 时再选。

配置文件会保存到：

```text
~/.config/gwtm/config.toml
```

示例：

```toml
projects_root_dir = "/Users/you/code"
worktrees_root_dir = "/Users/you/worktrees"
ide_mode = "app"
ide_command = "IntelliJ IDEA"
ide_label = "IntelliJ IDEA"
```

## 主菜单

- `创建 Worktree`
- `打开 Worktree`
- `列出 Worktree`
- `删除 Worktree`
- `重新配置`
- `退出程序`

## 说明

- Worktree 默认创建在：

```text
<worktrees_root_dir>/<project_name>/<branch_name>
```

- 分支名中的 `/` 会映射成目录名中的 `-`
- 默认工作流更偏向 Rust 项目，但工具本身适用于任意 Git 仓库

## 发版文档

- English: [`RELEASING.md`](RELEASING.md)
- 简体中文: [`RELEASING.zh-CN.md`](RELEASING.zh-CN.md)

## License

MIT
