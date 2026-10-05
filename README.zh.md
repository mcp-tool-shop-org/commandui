<p align="center">
  <a href="README.ja.md">日本語</a> | <a href="README.md">English</a> | <a href="README.es.md">Español</a> | <a href="README.fr.md">Français</a> | <a href="README.hi.md">हिन्दी</a> | <a href="README.it.md">Italiano</a> | <a href="README.pt-BR.md">Português (BR)</a>
</p>

<p align="center">
  <img src="https://raw.githubusercontent.com/mcp-tool-shop-org/brand/main/logos/commandui/readme.png" width="400" alt="CommandUI" />
</p>

<p align="center">
  <a href="https://github.com/mcp-tool-shop-org/commandui/actions/workflows/ci.yml"><img src="https://github.com/mcp-tool-shop-org/commandui/actions/workflows/ci.yml/badge.svg" alt="CI" /></a>
  <a href="https://codecov.io/gh/mcp-tool-shop-org/commandui"><img src="https://codecov.io/gh/mcp-tool-shop-org/commandui/graph/badge.svg" alt="Coverage" /></a>
  <a href="https://github.com/mcp-tool-shop-org/commandui/releases/latest"><img src="https://img.shields.io/github/v/release/mcp-tool-shop-org/commandui?label=Release" alt="Release" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue" alt="MIT License" /></a>
  <a href="https://mcp-tool-shop-org.github.io/commandui/"><img src="https://img.shields.io/badge/Landing_Page-live-blue" alt="Landing Page" /></a>
  <a href="https://mcp-tool-shop-org.github.io/commandui/handbook/"><img src="https://img.shields.io/badge/Handbook-read-blue" alt="Handbook" /></a>
</p>

一个为那些无法使用终端的人提供的界面。CommandUI 用通俗易懂的语言解释每个结果，允许你用通俗易懂的语言提出命令请求，并且在你查看并批准之前，不会执行任何草拟的命令。

## 目标用户

- 使用屏幕阅读器或不使用鼠标的用户
- 视力较弱，需要更大的文本或高对比度主题的用户
- 觉得终端操作困难的用户，包括初学者和有认知或学习障碍的用户
- 任何想要在命令执行前阅读命令的人

你仍然可以获得一个真正的 shell，拥有自己的配置文件和多个会话。输入命令的方式与以往一样。

## 安装

- **Microsoft Store：**[CommandUI 在 Microsoft Store 上的版本](https://apps.microsoft.com/detail/9NTN1GFQJ91M)。在本次更新发布到 Store 之前，Store 上会提供一个较早的版本。
- **直接下载：**从 [GitHub Releases](https://github.com/mcp-tool-shop-org/commandui/releases/latest) 下载 `.msi` 或安装程序 `.exe`。这些文件未进行代码签名，因此 Windows 可能会在安装之前要求您确认。
- **winget：**使用 `winget install mcp-tool-shop.CommandUI` 安装 v1.0.0，直到 winget 目录更新到 v1.0.2。

Windows 10 或 11，x64。Ask 功能需要同一台计算机上安装 `qwen2.5:14b` 模型。其他所有功能无需安装即可工作。

## 功能

- **每个结果都用一句话描述。**“完成。输出 3 行。”或“执行失败（退出代码 1）。该命令中的文件或文件夹不存在。”失败时会提供 **询问如何修复** 和 **再次运行** 选项。
- **用通俗易懂的语言提出请求。**描述任务，CommandUI 会草拟一个命令，解释它，然后等待。**运行计划** 表示批准，**拒绝** 则不执行任何操作。如果它无法解释一个命令，它会说明。
- **谨慎的确认。**删除文件或需要更高权限的命令，会在你输入文件夹名称之前等待。
- **命令仍然会执行你输入的内容。**如果一行看起来像一个请求，CommandUI 会提供使用 Ask 功能，而不是执行该语句。
- **你可以创建的工作流程。**创建一个命令列表，编辑它，运行它，然后删除它。删除操作可以撤销。历史记录可以保存你选择的命令。
- **你可以控制的历史记录和记忆。**搜索已执行的内容，并阅读或删除 CommandUI 记录的内容。

## 专为键盘和屏幕阅读器设计

- 结果和错误会一次性显示，而不会移动焦点。
- **输出**（Ctrl+Shift+O）将每个命令的输出以纯文本形式列出，每个命令一个区域，不包含终端代码。
- **F1** 打开键盘帮助。**Ctrl+Shift+R** 跳转到上一个结果。**Ctrl+Shift+A** 在“命令”和“询问”之间切换。
- 每个对话框都会保持焦点在其内部，并且按下 Esc 键会关闭它并返回焦点到你之前的位置。
- 文本大小可以在设置中从 100% 调整到 200%。终端下方的面板可以隐藏。
- Windows 对比度主题和减少动画设置都会被尊重。

**尚未测试的内容：**Narrator、NVDA 和 Windows 对比度主题尚未由该版本中的用户进行测试。这些测试将在后续更新中进行。在此之前，请将上述列表视为该应用程序的功能，而不是经过测试的声明。

## 安全性

CommandUI 在你的计算机上运行。它将历史记录、计划、工作流程、记忆和设置保存在本地，并且仅执行你批准的 shell 命令。它不会发送任何遥测数据。Ask 功能与此计算机上的模型进行通信。如果该模型未安装、未运行或未下载，Ask 功能会说明，并且不会草拟命令。

有关威胁模型以及如何报告漏洞，请参阅 [SECURITY.md](SECURITY.md)。

## 它不是

- 它不是一个聊天机器人，也不是一个可以自行运行草拟命令的程序。
- 它不是一个声明，即屏幕阅读器或对比度主题已在此版本中进行测试（参见上文）。
- 它不是控制台。`apps/console` 是此存储库中的第二个前端，不属于你安装的应用程序。

## 针对开发人员

```bash
pnpm install
pnpm dev          # browser preview; does not run your shell
pnpm test         # all tests
pnpm typecheck

# Rust
cd apps/desktop/src-tauri
cargo test
```

从发布版本打包 Store 上传：

```powershell
./packaging/build-store-exe.ps1
./packaging/pack-msix.ps1
```

`pack-msix.ps1` 写入 `release/CommandUI_<version>_x64.msix`。它保留了现有 Store 产品中的包名称、发布者和可执行文件，并且拒绝提交的版本低于上次提交的版本。该文件未签名；Partner Center 会对其进行签名。

```
commandui/
  apps/desktop/                 — the desktop app you install
  apps/console/                 — Rust terminal front end on the same runtime
  crates/runtime-core/          — shell sessions and events
  crates/runtime-persistence/   — local storage
  crates/runtime-planner/       — the local model Ask uses
  packages/                     — shared types, contracts, state, UI
  packaging/msix/               — Store manifest and logos
```

更多信息：[Handbook](https://mcp-tool-shop-org.github.io/commandui/handbook/) · [Developer Setup](docs/product/developer-setup.md) · [Known Limitations](docs/product/known-limitations.md) · [Release Checklist](docs/product/release-checklist.md)

## 状态

v1.0.2 已经在 GitHub Releases 上。Microsoft Store 上的更新正在进行认证。与 Narrator、NVDA 和 Windows 对比主题的测试将在后续更新中进行。

由 [MCP Tool Shop](https://mcp-tool-shop.github.io/) 构建。
