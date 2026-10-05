---
title: "Input Modes: Command vs Ask"
description: "Command runs what you type. Ask drafts a command and waits for Run Plan."
sidebar:
  order: 4
---

Both use the command box at the bottom. Ctrl+Shift+A switches them. CommandUI does not guess which one you meant.

## Command

Your text is sent to the shell. Use it when you know the command.

If the line looks like a request, CommandUI asks "This looks like a request. Ask CommandUI instead?" and does not run the sentence until you choose.

## Ask

Your text is a request. Ask drafts one command and opens the plan. You can edit it, choose Reject, or choose Run Plan. Nothing from Ask runs until you choose Run Plan.

If the model is not installed, not running, or not downloaded, Ask says which one and what to do next. It does not draft a command in a release build.

A debug build, and the browser preview used while developing, can show a practice plan. That plan says "Practice plan — Ollama is not connected. This is not a real plan."

## While a command runs

The button says Stop.

## Typing in the terminal

Keystrokes in the terminal go straight to the shell. They are not listed in History. Use the command box for a command you want recorded.
