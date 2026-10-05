<p align="center">
  <a href="README.md">English</a> | <a href="README.zh.md">中文</a> | <a href="README.es.md">Español</a> | <a href="README.fr.md">Français</a> | <a href="README.hi.md">हिन्दी</a> | <a href="README.it.md">Italiano</a> | <a href="README.pt-BR.md">Português (BR)</a>
</p>

<p align="center">
  <img src="https://raw.githubusercontent.com/mcp-tool-shop-org/brand/main/logos/commandui/readme.png" width="400" alt="CommandUI" />
</p>

<p align="center">
  <a href="https://github.com/mcp-tool-shop-org/commandui/actions/workflows/ci.yml"><img src="https://github.com/mcp-tool-shop-org/commandui/actions/workflows/ci.yml/badge.svg" alt="CI" /></a>
  <a href="https://github.com/mcp-tool-shop-org/commandui/releases/latest"><img src="https://img.shields.io/github/v/release/mcp-tool-shop-org/commandui?label=Release" alt="Release" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue" alt="MIT License" /></a>
  <a href="https://mcp-tool-shop-org.github.io/commandui/"><img src="https://img.shields.io/badge/Landing_Page-live-blue" alt="Landing Page" /></a>
  <a href="https://mcp-tool-shop-org.github.io/commandui/handbook/"><img src="https://img.shields.io/badge/Handbook-read-blue" alt="Handbook" /></a>
</p>

ターミナルを使いにくいと感じている人向けのシェル。CommandUIは、すべての結果をわかりやすい言葉で説明し、わかりやすい言葉でコマンドを尋ねることを可能にし、コマンドを実行する前に、その内容を確認して承認するまで、コマンドの実行を保留します。

## 対象ユーザー

- スクリーンリーダーを使用している人、またはマウスを使用しない人
- 視覚障碍のある人で、より大きなテキストやコントラストの高いテーマが必要な人
- ターミナルを使いにくいと感じている人（初心者や、認知または学習障害のある人を含む）
- コマンドを実行する前に、その内容を確認したい人

それでも、独自のプロファイルと複数のセッションを持つ、通常のシェルを使用できます。コマンドを入力すると、これまで通りに動作します。

## インストール

- **Microsoft Store:** [Microsoft StoreのCommandUI](https://apps.microsoft.com/detail/9NTN1GFQJ91M)。現在、ストアには古いバージョンがあります。ここに記載されているアップデートは、アクセシビリティテストが完了した後、提出される予定です。
- **winget:** `winget install mcp-tool-shop.CommandUI` を使用して、[GitHub Releases](https://github.com/mcp-tool-shop-org/commandui/releases/latest) からv1.0.0をインストールします。

Windows 10または11、x64。Ask機能を使用するには、同じコンピューターに`qwen2.5:14b`モデルと[Ollama](https://ollama.com)が必要です。それ以外は、それらがなくても動作します。

## 機能

- **すべての結果を1つの文で表示。** 「完了。3行の出力。」または「エラーが発生しました（終了コード1）。指定されたコマンド内のファイルまたはフォルダーが存在しません。」エラーが発生した場合、**修正方法を尋ねる**、および**再実行する**オプションが表示されます。
- **わかりやすい言葉で質問。** タスクを説明すると、CommandUIはコマンドを作成し、説明し、実行を待ちます。**実行**ボタンを押すとコマンドが実行され、**拒否**ボタンを押すと何も実行されません。CommandUIがコマンドを説明できない場合、その旨を伝えます。
- **慎重な確認。** ファイルを削除したり、より高い権限を必要とするコマンドは、フォルダー名を入力するまで実行されません。
- **コマンドは、入力した内容を実行します。** 入力された内容がリクエストのように見える場合、CommandUIは実行する代わりに、Ask機能を使用することを提案します。
- **作成できるワークフロー。** コマンドのリストを作成し、編集し、実行し、削除できます。削除操作は元に戻すことができます。履歴にコマンドを保存することもできます。
- **制御可能な履歴とメモリ。** 実行されたコマンドを検索し、CommandUIが記録した内容を読み取ったり、削除したりできます。

## キーボードとスクリーンリーダー向けに設計

- 結果とエラーは1回だけ通知され、フォーカスが移動することはありません。
- **出力**（Ctrl+Shift+O）は、各コマンドの出力をプレーンテキストの形式で、コマンドごとに1つの領域として表示します。ターミナルコードは含まれません。
- **F1**を押すと、キーボードヘルプが開きます。**Ctrl+Shift+R**を押すと、最後の結果にジャンプします。**Ctrl+Shift+A**を押すと、コマンドとAskの間で切り替わります。
- すべてのダイアログは、ダイアログ内にフォーカスを維持し、Escapeキーを押すとダイアログが閉じ、フォーカスが元の位置に戻ります。
- テキストサイズは、設定で100%から200%に調整できます。ターミナルの下にあるパネルは非表示にできます。
- Windowsのコントラストテーマと、モーションを減らす設定が適用されます。

**まだテストされていないもの:** Narrator、NVDA、およびWindowsのコントラストテーマは、このビルドでテストされていません。これらのテストは、ストアへのアップデートの前に実施されます。それまでの間、上記のリストは、アプリがどのような機能を提供するように設計されているかを示すものであり、テスト済みの機能であるという主張ではありません。

## セキュリティ

CommandUIは、ローカルマシンで実行されます。履歴、プラン、ワークフロー、メモリ、および設定はローカルに保存され、承認したシェルコマンドのみが実行されます。テレメトリは送信されません。Ask機能は、このコンピューター上のモデルと通信します。そのモデルがインストールされていない、実行されていない、またはダウンロードされていない場合、Ask機能はそれを通知し、コマンドを作成しません。

脅威モデルと、脆弱性を報告する方法については、[SECURITY.md](SECURITY.md)を参照してください。

## これは何ではないか

- チャットボットではなく、作成されたコマンドを自動的に実行するものではありません。
- スクリーンリーダーまたはコントラストテーマがこのビルドでテストされているという主張ではありません（上記を参照）。
- コンソールではありません。`apps/console`は、このリポジトリにある別のフロントエンドであり、インストールするアプリの一部ではありません。

## 開発者向け

```bash
pnpm install
pnpm dev          # browser preview; does not run your shell
pnpm test         # all tests
pnpm typecheck

# Rust
cd apps/desktop/src-tauri
cargo test
```

リリースビルドからストアへのアップロードをパッケージ化します。

```powershell
./packaging/build-store-exe.ps1
./packaging/pack-msix.ps1
```

`pack-msix.ps1` は `release/CommandUI_<version>_x64.msix` を書き込みます。既存のストア製品のパッケージ名、発行元、および実行ファイルを保持し、最後に送信されたバージョンよりも古いバージョンは拒否します。ファイルは署名されていません。パートナーセンターで署名されます。

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

詳細: [Handbook](https://mcp-tool-shop.github.io/commandui/handbook/) · [Developer Setup](docs/product/developer-setup.md) · [Known Limitations](docs/product/known-limitations.md) · [Release Checklist](docs/product/release-checklist.md)

## ステータス

v1.0.2、まだリリースされていません。Microsoft Storeには古いバージョンがあり、GitHubの公開リリースはv1.0.0です。

[MCP Tool Shop](https://mcp-tool-shop.github.io/)によって作成されました。
