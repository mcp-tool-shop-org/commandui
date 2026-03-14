<p align="center">
  <a href="README.ja.md">日本語</a> | <a href="README.zh.md">中文</a> | <a href="README.es.md">Español</a> | <a href="README.md">English</a> | <a href="README.hi.md">हिन्दी</a> | <a href="README.it.md">Italiano</a> | <a href="README.pt-BR.md">Português (BR)</a>
</p>

<p align="center">
  <img src="https://raw.githubusercontent.com/mcp-tool-shop-org/brand/main/logos/commandui/readme.png" width="400" alt="CommandUI" />
</p>

# CommandUI

Environnement de shell natif intégrant une analyse sémantique des commandes.

## Fonctionnalités

- Sessions de shell PTY réelles (et non une simple enveloppe ou un chatbot)
- Deux modes de saisie : saisie directe dans le terminal (libre) + éditeur (structuré/traçable)
- Mode sémantique : description de l'intention → l'IA génère la commande → vous la vérifiez/modifiez/approuvez
- Confirmation par niveau de risque : faible (automatique), moyen (configurable), élevé (obligatoire)
- Historique avec les actions "relancer", "réouvrir le plan" et "enregistrer dans le flux de travail"
- Flux de travail enregistrés : transformez n'importe quelle commande en un flux de travail réutilisable
- Mémoire spécifique au projet : apprend les préférences à partir des modifications répétées
- Onglets pour plusieurs sessions avec flux de terminal par session
- Persistance locale SQLite (historique, plans, flux de travail, mémoire, paramètres)
- Modes "classique" et "guidé" avec des différences de comportement réelles

## Ce que ce n'est PAS

- Ce n'est pas un chatbot ou un agent autonome.
- Ce n'est pas un remplacement d'émulateur de terminal.
- Ce n'est pas une version stable (version préliminaire v0).

## Disposition de l'espace de travail

```
commandui/
  apps/desktop/         — Tauri v2 + React 19 desktop app
  packages/domain/      — Pure domain types
  packages/api-contract/ — Request/response contracts
  packages/state/       — Zustand stores
  packages/ui/          — Shared UI primitives (future)
```

## Démarrage rapide

```bash
pnpm install
pnpm dev          # Vite dev server
pnpm test         # Run all tests
pnpm typecheck    # TypeScript check

# Rust backend
cd apps/desktop/src-tauri
cargo test
```

## Documentation

- [Configuration pour les développeurs](docs/product/developer-setup.md)
- [Limitations connues](docs/product/known-limitations.md)
- [Liste de contrôle des tests de base](docs/specs/smoke-test-checklist.md)
- [Liste de contrôle de la publication](docs/product/release-checklist.md)

## État actuel

Version préliminaire v0 avec un noyau de shell réel. Ensemble de fonctionnalités de base comprenant : sessions PTY, boucle d'analyse sémantique, persistance, mémoire, flux de travail, paramètres d'accessibilité, onglets pour plusieurs sessions, terminal xterm.js, détection de complétion des invites.
