# Changelog

All notable changes to this project will be documented in this file.

## [1.0.2] - 2026-10-03

### Added
- Unsigned x64 MSIX for the existing Partner Center product. Package name `mcp-tool-shop.CommandUI`, package version `1.0.2.0`. `packaging/build-store-exe.ps1` builds the executable with the user-profile prefix remapped out. `packaging/pack-msix.ps1` packs it and refuses a name, publisher, architecture, executable, or version that would not update that product. The file is unsigned. Partner Center signs it.

### Fixed
- A promoted workflow keeps its step list. The desktop writes `stepsJson` and restores `steps` on launch, so a restart still runs the steps one at a time. A save from the plan panel or from history stores that one command.

### Changed
- Desktop version is 1.0.2. The public GitHub release is still the v1.0.0 MSI until the Store upload is published.
- README, landing page, and the beginner handbook page describe the MSIX as the Store package and the MSI as the current direct download.
- Known limitations describe the real planner: local Ollama, then a mock fallback.

## [1.0.0] - 2026-03-13

### Added
- Error boundary with copy-error and reload recovery
- Boot resilience state machine (booting → ready | failed)
- Mock planner indicator in plan panel
- Global unhandled rejection handler
- Full Treatment: landing page with site-theme + Starlight handbook (26 chapters)
- Translations (ja, zh, es, fr, hi, it, pt-BR)
- SECURITY.md with threat model
- LICENSE (MIT)

### Changed
- Version bump from v0.0.1 to v1.0.0 across all packages
- Mock bridge returns `source: "mock"` (was `"semantic"`)

## [0.0.1] - 2026-03-12

### Added
- Phase 1–7: PTY sessions, semantic input, plan review, history, workflows, memory, settings
- Multi-session tabs with per-session terminal streams
- Risk-tiered confirmation (low/medium/high)
- Workflow promotion from repeated command sequences
- Project-scoped memory with confidence scoring
- Classic vs Guided modes
- xterm.js terminal with prompt-marker completion detection
- Local-first SQLite persistence
- Keyboard velocity and session-scoped audit trail
- Planner context enrichment with workflow awareness
- Workflow run inspection and cross-drawer navigation
