---
title: What Is CommandUI
description: Overview of CommandUI — an AI-native shell that translates intent into commands you control.
sidebar:
  order: 1
---

CommandUI is an AI-native shell environment. It gives you a real terminal with a second input path: describe what you want in natural language, and an AI planner translates your intent into a shell command. You review, edit, approve, or reject before anything executes.

## What it is

- A desktop app (Tauri v2 + React 19) with a real PTY shell
- Two input paths: direct terminal typing and structured composer
- Semantic mode: intent in, command plan out, you decide
- Risk-tiered confirmation: low-risk commands flow, high-risk commands require acknowledgment
- History with rerun, plan inspection, and workflow saving
- Project-scoped memory that learns your preferences from repeated edits
- Multi-session tabs with independent terminal streams
- Local-first persistence via SQLite

## What it is not

- Not a chatbot. It does not converse. It translates intent to commands.
- Not an autonomous agent. Nothing executes without your approval.
- Not a terminal emulator replacement. It wraps a real shell, it does not simulate one.
- Not a cloud service. Everything runs locally. Your data stays on disk.

## The core promise

You always know three things:

1. **What you asked** — your original intent is preserved
2. **What the system plans to do** — the exact command, with explanation and risk assessment
3. **How to stop it** — reject, edit, or close the plan before execution

If any screen state hides one of those three, it is a bug.

## The bidirectional moat

CommandUI works in both directions:

- **Beginners:** describe intent in natural language, see the real command, learn what it does
- **Experts:** type commands directly, save proven sequences as workflows, accelerate repetitive work

This is the product thesis. A shell that teaches and a shell that accelerates are the same shell.
