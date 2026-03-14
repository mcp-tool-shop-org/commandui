<p align="center">
  <a href="README.ja.md">日本語</a> | <a href="README.zh.md">中文</a> | <a href="README.es.md">Español</a> | <a href="README.fr.md">Français</a> | <a href="README.hi.md">हिन्दी</a> | <a href="README.it.md">Italiano</a> | <a href="README.md">English</a>
</p>

<p align="center">
  <img src="https://raw.githubusercontent.com/mcp-tool-shop-org/brand/main/logos/commandui/readme.png" width="400" alt="CommandUI" />
</p>

# CommandUI

Ambiente de shell nativo com revisão semântica de comandos.

## O que ele faz

- Sessões de shell PTY reais (não é um wrapper, nem um chatbot)
- Dois caminhos de entrada: digitação direta no terminal (livre) + editor (estruturado/rastreado)
- Modo semântico: descreva a intenção → a IA gera o comando → você revisa/edita/aprova
- Confirmação em níveis de risco: baixo (automático), médio (configurável), alto (obrigatório)
- Histórico com ações de repetição, reabrir plano e salvar no fluxo de trabalho
- Fluxos de trabalho salvos: transforme qualquer comando em um fluxo de trabalho reutilizável
- Memória específica do projeto: aprende preferências a partir de edições repetidas
- Abas de várias sessões com fluxos de terminal por sessão
- Persistência SQLite local (histórico, planos, fluxos de trabalho, memória, configurações)
- Modos clássico e guiado com diferenças comportamentais reais

## O que ele NÃO é

- Não é um chatbot ou agente autônomo
- Não é um substituto para um emulador de terminal
- Não é uma versão finalizada para produção (versão inicial v0)

## Layout do espaço de trabalho

```
commandui/
  apps/desktop/         — Tauri v2 + React 19 desktop app
  packages/domain/      — Pure domain types
  packages/api-contract/ — Request/response contracts
  packages/state/       — Zustand stores
  packages/ui/          — Shared UI primitives (future)
```

## Como começar

```bash
pnpm install
pnpm dev          # Vite dev server
pnpm test         # Run all tests
pnpm typecheck    # TypeScript check

# Rust backend
cd apps/desktop/src-tauri
cargo test
```

## Documentação

- [Configuração para desenvolvedores](docs/product/developer-setup.md)
- [Limitações conhecidas](docs/product/known-limitations.md)
- [Lista de verificação de teste inicial](docs/specs/smoke-test-checklist.md)
- [Lista de verificação de lançamento](docs/product/release-checklist.md)

## Status atual

Versão inicial v0 com um núcleo de shell real. Um conjunto de 21 componentes oferece: sessões PTY, ciclo de revisão semântica, persistência, memória, fluxos de trabalho, configurações de acessibilidade, abas de várias sessões, terminal xterm.js, detecção de conclusão de marcadores de prompt.
