# Changelog

All notable changes to this project will be documented in this file.

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
