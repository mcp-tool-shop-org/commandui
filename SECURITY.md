# Security Policy

## Supported versions

| Version | Supported |
|---------|-----------|
| 1.x     | Yes       |

## Reporting a vulnerability

If you discover a security vulnerability, please report it privately via
[GitHub Security Advisories](https://github.com/mcp-tool-shop-org/commandui/security/advisories/new).

Do **not** open a public issue for security reports.

## Threat model

CommandUI is a desktop shell that runs commands on behalf of the user.
The primary security boundaries are:

| Boundary | Control |
|----------|---------|
| AI-generated commands execute on your machine | Risk-tiered confirmation: low (auto-run), medium (configurable), high (always confirm). Nothing destructive runs without explicit approval. |
| Local LLM communication | Ollama runs locally — no credentials or shell content leave your machine. |
| Persisted data (SQLite) | History, plans, workflows, and memory are stored in a local SQLite database. No cloud sync. No telemetry. |
| Tauri IPC | Frontend communicates with the Rust backend via Tauri's typed invoke API. No arbitrary shell access from the webview. |
| Dependencies | Standard npm + Cargo supply chain. No post-install scripts in first-party packages. |

## What CommandUI does NOT do

- Does not send data to external servers
- Does not collect telemetry or analytics
- Does not auto-update without user action
- Does not execute commands without user review (in Guided mode)
- Does not store credentials or secrets
