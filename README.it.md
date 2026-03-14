<p align="center">
  <a href="README.ja.md">日本語</a> | <a href="README.zh.md">中文</a> | <a href="README.es.md">Español</a> | <a href="README.fr.md">Français</a> | <a href="README.hi.md">हिन्दी</a> | <a href="README.md">English</a> | <a href="README.pt-BR.md">Português (BR)</a>
</p>

<p align="center">
  <img src="https://raw.githubusercontent.com/mcp-tool-shop-org/brand/main/logos/commandui/readme.png" width="400" alt="CommandUI" />
</p>

# CommandUI

Ambiente shell nativo con revisione semantica dei comandi.

## Cosa fa

- Sessioni shell PTY reali (non un wrapper, non un chatbot)
- Due modalità di input: digitazione diretta nel terminale (libera) + compositore (strutturata/tracciata)
- Modalità semantica: descrivere l'intento → l'IA genera il comando → l'utente lo revisiona/modifica/approva
- Conferma a livelli di rischio: basso (automatico), medio (configurabile), alto (obbligatorio)
- Cronologia con azioni di ripetizione, riapertura del piano e salvataggio nel flusso di lavoro
- Flussi di lavoro salvati: promuovere qualsiasi comando a un flusso di lavoro riutilizzabile
- Memoria specifica per progetto: impara le preferenze dalle modifiche ripetute
- Schede per sessioni multiple con flussi di terminale per sessione
- Persistenza SQLite locale (cronologia, piani, flussi di lavoro, memoria, impostazioni)
- Modalità classica e guidata con differenze comportamentali reali

## Cosa NON è

- Non è un chatbot o un agente autonomo
- Non è un sostituto di un emulatore di terminale
- Non è una versione stabile (versione iniziale v0)

## Layout dell'area di lavoro

```
commandui/
  apps/desktop/         — Tauri v2 + React 19 desktop app
  packages/domain/      — Pure domain types
  packages/api-contract/ — Request/response contracts
  packages/state/       — Zustand stores
  packages/ui/          — Shared UI primitives (future)
```

## Guida rapida

```bash
pnpm install
pnpm dev          # Vite dev server
pnpm test         # Run all tests
pnpm typecheck    # TypeScript check

# Rust backend
cd apps/desktop/src-tauri
cargo test
```

## Documentazione

- [Configurazione per sviluppatori](docs/product/developer-setup.md)
- [Limitazioni note](docs/product/known-limitations.md)
- [Checklist dei test preliminari](docs/specs/smoke-test-checklist.md)
- [Checklist per il rilascio](docs/product/release-checklist.md)

## Stato attuale

Versione iniziale v0 con un nucleo shell reale. Un pacchetto di 21 componenti fornisce: sessioni PTY, ciclo di revisione semantica, persistenza, memoria, flussi di lavoro, impostazioni di accessibilità, schede per sessioni multiple, terminale xterm.js, rilevamento del completamento con indicatori.
