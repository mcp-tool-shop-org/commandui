<p align="center">
  <a href="README.md">English</a> | <a href="README.zh.md">中文</a> | <a href="README.es.md">Español</a> | <a href="README.fr.md">Français</a> | <a href="README.hi.md">हिन्दी</a> | <a href="README.it.md">Italiano</a> | <a href="README.pt-BR.md">Português (BR)</a>
</p>

<p align="center">
  <img src="https://raw.githubusercontent.com/mcp-tool-shop-org/brand/main/logos/commandui/readme.png" width="400" alt="CommandUI" />
</p>

# CommandUI

意味解析に基づくコマンドレビュー機能を備えた、AIネイティブなシェル環境。

## 機能概要

- 実際のPTYシェルセッション（ラッパーやチャットボットではありません）
- 2つの入力方法：直接ターミナルへの入力（自由形式）＋コンポーザー（構造化/追跡可能）
- セマンティックモード：意図を記述 → AIがコマンドを生成 → ユーザーがレビュー/編集/承認
- リスクレベルに応じた確認：低（自動）、中（設定可能）、高（必須）
- 履歴機能：再実行、プラン再開、ワークフローへの保存
- 保存されたワークフロー：任意のコマンドを再利用可能なワークフローとして登録
- プロジェクトごとの記憶機能：繰り返しの編集から好みを学習
- マルチセッションタブ：各セッションごとにターミナルのストリームを表示
- ローカルファーストのSQLiteによる永続化（履歴、プラン、ワークフロー、記憶、設定）
- クラシックモードとガイドモード：実際の動作に違いがある

## これはNOT

- チャットボットや自律エージェントではありません
- ターミナルエミュレーターの代替ではありません
- 本番環境での利用を想定したものではありません（初期段階のv0）

## ワークスペースのレイアウト

```
commandui/
  apps/desktop/         — Tauri v2 + React 19 desktop app
  packages/domain/      — Pure domain types
  packages/api-contract/ — Request/response contracts
  packages/state/       — Zustand stores
  packages/ui/          — Shared UI primitives (future)
```

## クイックスタート

```bash
pnpm install
pnpm dev          # Vite dev server
pnpm test         # Run all tests
pnpm typecheck    # TypeScript check

# Rust backend
cd apps/desktop/src-tauri
cargo test
```

## ドキュメント

- [開発者向けセットアップ](docs/product/developer-setup.md)
- [既知の制限事項](docs/product/known-limitations.md)
- [初期動作確認チェックリスト](docs/specs/smoke-test-checklist.md)
- [リリースチェックリスト](docs/product/release-checklist.md)

## 現在の状況

実際のシェル機能を備えた初期段階のv0。21の主要コンポーネントで構成：PTYセッション、セマンティックレビューループ、永続化、記憶機能、ワークフロー、アクセシビリティ設定、マルチセッションタブ、xterm.jsターミナル、プロンプトマーカーによる補完検出。
