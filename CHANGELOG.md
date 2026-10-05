# Changelog

All notable changes to this project will be documented in this file.

## [1.0.2] - 2026-10-05

The functional update. It makes no claim of tested accessibility: testing with Narrator, NVDA and Windows contrast themes comes in a later update.

### Added
- Every command ends in one plain sentence that says whether it worked, and what to do if it did not. A failure offers **Ask how to fix it** and **Run again**.
- **Output** (Ctrl+Shift+O) lists each command as plain text, one named region per command, with no terminal codes.
- Results are announced once in a polite status message; errors are announced as alerts.
- A keyboard path for every action. F1 opens keyboard and reading help. Ctrl+Shift+R moves to the last result, Ctrl+Shift+A switches between Command and Ask.
- Text size from 100% to 200% in Settings, and panels below the terminal that can be hidden.
- A welcome screen on first open, and the new logo.
- Workflows can be created, edited, run and undone from the Workflows drawer.
- A plan that deletes files or needs higher permissions waits until you type the folder name.
- Unsigned x64 MSIX for the existing Partner Center product (`mcp-tool-shop.CommandUI`, `1.0.2.0`). `packaging/build-store-exe.ps1` builds the executable with the user-profile prefix remapped out; `packaging/pack-msix.ps1` refuses a name, publisher, architecture, executable or version that would not update that product.
- Codecov line coverage for Rust and TypeScript, each gated at 90%.

### Changed
- PowerShell sessions start without the bootstrap echo, in your home folder when the process starts in System32.
- Folders are shown with your home folder as `~` in the header, History, Memory and suggestions, so the account name stays off the screen.
- Plan risk, flags and memory suggestions are written as sentences ("CommandUI is 65% sure.", "Seen in 6 commands you ran.").
- Only a high-risk plan asks for confirmation; the medium-risk setting and Reduced clutter are gone.
- The first Ask after CommandUI opens waits for the local model to load (up to 100 s) and says so after 5 s, instead of failing after 15 s.

### Fixed
- A promoted workflow keeps its step list across a restart.
- Text at 200% no longer collapses the main screen.
- The workflow editor ignores a second click on Create or Save.

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
