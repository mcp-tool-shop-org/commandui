---
title: For Beginners
description: New to CommandUI? Start here for a gentle introduction.
sidebar:
  order: 99
---

New to CommandUI or AI-assisted shells? This page explains everything from scratch.

## What is this tool?

**CommandUI** is a desktop application that gives you a real terminal shell with an AI assistant built in. You can type commands directly (like any terminal), or describe what you want in plain English and let the AI suggest the right command for you to review before running.

The key idea: you always stay in control. The AI proposes, you approve. Nothing runs without your explicit action.

## Who is this for?

- **Developers** who want AI help writing shell commands without giving up control
- **System administrators** who want a safer way to run complex commands with risk awareness
- **Terminal users** who know what they want to accomplish but don't always remember exact syntax
- **Anyone** who wants command history, saved workflows, and learned preferences in their shell

You don't need to be an AI expert. If you can use a terminal, you can use CommandUI.

## Prerequisites

Before you start, you need:

- **Windows 10 or later** — CommandUI is a native Windows desktop app built with Tauri v2
- **A terminal-compatible shell** — PowerShell or Git Bash (CommandUI spawns a real PTY process)
- **Ollama** (optional) — for AI-powered command generation. Without Ollama, CommandUI falls back to a mock planner that echoes your intent. Install from [ollama.com](https://ollama.com/) if you want the full semantic experience

You do NOT need:
- Programming experience (though terminal familiarity helps)
- An internet connection for AI features (Ollama runs locally)
- Any API keys or cloud accounts

## Your First 5 Minutes

### 1. Install CommandUI

Download the MSI installer from [GitHub Releases](https://github.com/mcp-tool-shop-org/commandui/releases/latest) or use Scoop:

```powershell
scoop bucket add mcp-tool-shop https://github.com/mcp-tool-shop-org/scoop-bucket
scoop install commandui
```

### 2. Launch and run a raw command

Open CommandUI. You'll see a terminal area on the left and a plan panel on the right. The composer at the bottom starts in **Command** mode.

Click the composer (or press `Ctrl+J`), type `ls`, and press Enter. The command runs in a real shell — output appears in the terminal just like any other terminal app.

### 3. Try the AI assistant

Click the **Ask** button (or press `Ctrl+2` while in the composer) to switch to Ask mode. Type "show me all files modified today" and press Enter.

Instead of running immediately, CommandUI generates a **plan** — a proposed command with an explanation and risk level. You can:
- **Run Plan** to execute it
- **Edit** the command before running
- **Reject** to dismiss it entirely

### 4. Check your history

Press `Ctrl+H` to open the history drawer. Every command you've run (or rejected) is listed with its status, duration, and source. You can rerun any command from history.

### 5. Save a workflow

Found a command you'll use again? Click **Save Workflow** on any history item or plan to save it for quick reuse later.

## Common Mistakes

### 1. Expecting a chatbot
CommandUI is not a chatbot. It doesn't have multi-turn conversations. Each Ask mode submission generates a single command proposal. If the first suggestion isn't right, reject it and rephrase your intent.

### 2. Forgetting which mode you're in
If you type a natural language description in Command mode, it will try to run your sentence as a shell command (which will fail). Check the mode indicator at the bottom — **Command** sends raw commands, **Ask** generates plans.

### 3. Not installing Ollama
Without Ollama running locally, the AI features use a mock planner that just echoes your input. For real command generation, install Ollama and make sure it's running before launching CommandUI.

### 4. Trying to use it as a terminal replacement
CommandUI augments your terminal workflow — it doesn't replace your existing terminal. Direct terminal typing (clicking into the terminal area) works but bypasses structured history. Use the composer for tracked commands.

### 5. Ignoring risk tiers
When the plan panel shows **high risk** (destructive commands like `rm -rf`), take the warning seriously. Review the command carefully before approving. Risk tiers exist to protect you.

## Next Steps

- Follow the [First Ten Minutes](../first-ten-minutes/) walkthrough for a complete guided session
- Read [Core Concepts](../core-concepts/) to understand sessions, plans, and workflows
- Check [Input Modes](../input-modes/) for details on Command vs Ask mode
- Browse [Keyboard Shortcuts](../keyboard-shortcuts/) to speed up your workflow

## Glossary

| Term | Definition |
|---|---|
| **Composer** | The input area at the bottom of the screen where you type commands or intents |
| **Command mode** | Input mode that sends your text directly to the shell as a raw command |
| **Ask mode** | Input mode that sends your text to the AI planner for command generation |
| **Plan** | A structured proposal: AI-generated command with explanation, risk level, and review options |
| **PTY** | Pseudo-terminal — a real shell process (not a simulation), so commands behave exactly as they would in a normal terminal |
| **Risk tier** | A safety classification (low/medium/high) assigned to every generated plan |
| **Workflow** | A saved command or command sequence that you can rerun with one click |
| **Memory** | Learned preferences from your usage patterns (frequent directories, repeated commands) |
| **Ollama** | A local AI runtime that runs language models on your machine — used by CommandUI for command generation |
| **Session** | A single shell process with its own terminal stream, working directory, and state |
