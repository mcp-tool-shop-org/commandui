# Brand Rules

## Tone
Direct, mechanical, no filler. Error messages state what failed and what to do. Status messages describe state, not emotion. Key hints use caret notation (^T, ^G, ^Q) not prose ("press Ctrl+T"). Never apologize, never use exclamation marks in UI text, never anthropomorphize the shell.

## Domain Language
- **session** (not tab, window, instance) — a live PTY connection
- **run** (not process, job) — a session in the run selector context
- **ask** (not prompt, chat, query) — the intent composition mode
- **review** (not confirm, approve dialog) — the proposal inspection mode
- **proposal** (not suggestion, recommendation, plan) — a generated command with risk assessment
- **raw play** (not passthrough mode, fullscreen mode, game mode) — host terminal surrender
- **idle** (not ready, waiting) — session is yours, nothing running
- **sidecar** (not assistant, copilot, helper) — the AI's relationship to play

## Forbidden Metaphors
- No chat/conversation framing: "talk to", "ask me anything", "how can I help", "let's chat"
- No assistant personality: no names, no greetings, no "I think", no opinions
- No magic language: "smart", "intelligent", "understands", "learns", "thinks"
- No cloud/remote imagery: "sync", "connected", "online", "cloud-powered"
- No gaming-specific branding despite play support: Console is a shell, not a game launcher

## Truth Constraints
- Never claim AI-generated commands are safe — always show risk assessment
- Never imply Console replaces your shell — it wraps it
- Never claim real-time AI during raw play — AI is sidecar, not overlay
- Never imply Desktop and Console have identical features — they share law, not surface
- Never claim transcript fidelity during raw play — it's captured but degraded

## Contamination Risks
Desktop (Tauri + React) and Console (Ratatui) share crates but have different interaction models. Desktop has spatial layout, workflows UI, memory UI. Console has none of these yet. Do not describe Console capabilities using Desktop feature language. Do not port Desktop UI patterns to Console without doctrine review.

## Interaction Law (Terminal-Native Command Register)

CommandUI has its own interaction register. It is not "not-chat." It is command-native.

- **Every action respects terminal rhythm** — keystrokes are immediate, mode switches are instant, there are no transitions, animations, or "loading" states that block input. The shell is always responsive.
- **No decorative affordance language** — no "click here", no "tap to continue", no "swipe", no hover states. Controls are key chords. Discoverability is the help overlay and footer hints.
- **No GUI-style "workspace" framing** — no "your workspace", no "dashboard", no "home screen." You are in a shell. The session IS the workspace.
- **No fake safety/reassurance language** — no "don't worry", no "safely", no "we'll take care of it." Risk is stated as risk. Destructive is stated as destructive. The user decides.
- **No anthropomorphic command narration** — the planner does not "think", "consider", or "suggest." It generates a proposal. The proposal has a command, a risk level, and an explanation. That's it.
- **State language is operational, not conversational** — "idle", "running", "stopping", "starting", "done", "error." Not "waiting for you", "working on it", "almost there", "something went wrong."

## Approved Visual Register
Terminal-native. Monospace. Borders and badges, not icons. Color for state (green=idle, cyan=running, yellow=caution, red=error/done), not decoration. No ASCII art, no splash screens, no loading animations. The welcome banner is the most visual thing Console shows — and it's text.
