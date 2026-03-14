<p align="center">
  <a href="README.ja.md">日本語</a> | <a href="README.md">English</a> | <a href="README.es.md">Español</a> | <a href="README.fr.md">Français</a> | <a href="README.hi.md">हिन्दी</a> | <a href="README.it.md">Italiano</a> | <a href="README.pt-BR.md">Português (BR)</a>
</p>

<p align="center">
  <img src="https://raw.githubusercontent.com/mcp-tool-shop-org/brand/main/logos/commandui/readme.png" width="400" alt="CommandUI" />
</p>

# CommandUI

具有语义命令审查功能的、原生AI交互环境。

## 其功能

- 真正的PTY终端会话（不是简单的包装，也不是聊天机器人）
- 两种输入方式：直接终端输入（自由形式）+ 结构化/可跟踪的编辑器
- 语义模式：描述意图 → AI生成命令 → 您审查/编辑/批准
- 风险分级确认：低风险（自动）、中风险（可配置）、高风险（必须）
- 历史记录，支持重新运行、重新打开计划以及保存到工作流的操作
- 保存的工作流：可以将任何命令提升为可重用的工作流
- 项目范围内的记忆：从重复的编辑中学习用户偏好
- 多会话标签，每个会话都有独立的终端流
- 本地优先的SQLite持久化（历史记录、计划、工作流、记忆、设置）
- 经典模式与引导模式，具有实际的不同行为

## 它不是

- 并非聊天机器人或自主代理
- 并非终端模拟器的替代品
- 尚未经过生产环境的严格测试（早期版本v0）

## 工作区布局

```
commandui/
  apps/desktop/         — Tauri v2 + React 19 desktop app
  packages/domain/      — Pure domain types
  packages/api-contract/ — Request/response contracts
  packages/state/       — Zustand stores
  packages/ui/          — Shared UI primitives (future)
```

## 快速开始

```bash
pnpm install
pnpm dev          # Vite dev server
pnpm test         # Run all tests
pnpm typecheck    # TypeScript check

# Rust backend
cd apps/desktop/src-tauri
cargo test
```

## 文档

- [开发者设置](docs/product/developer-setup.md)
- [已知限制](docs/product/known-limitations.md)
- [初步测试清单](docs/specs/smoke-test-checklist.md)
- [发布清单](docs/product/release-checklist.md)

## 当前状态

早期版本v0，具有真正的终端核心。由21个组件组成的启动程序，提供以下功能：PTY会话、语义审查循环、持久化、记忆、工作流、可访问性设置、多会话标签、xterm.js终端、提示符标记自动完成检测。
