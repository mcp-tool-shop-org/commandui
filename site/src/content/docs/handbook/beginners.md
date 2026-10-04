---
title: For Beginners
description: What CommandUI is, how to run a command, and how to ask for one.
sidebar:
  order: 99
---

## What this is

CommandUI is a desktop app with a real shell. You can type a command, or describe what you want and approve the command it drafts. Nothing it drafts runs until you choose Run Plan.

It is aimed at people a bare terminal shuts out, and at anyone who wants the command written out before it runs. This page is not a test report. Screen-reader and high-contrast checks with a person are still open.

## Before you start

- Windows
- A shell on this computer, such as PowerShell
- A local model, if you want Ask to draft a real command. If it is not installed, not running, or not downloaded, Ask says so. It does not invent a command in the release app.

You do not need a cloud account.

## Install

The Store package is an MSIX named `mcp-tool-shop.CommandUI`. Until that upload is published, install the MSI from [GitHub Releases](https://github.com/mcp-tool-shop-org/commandui/releases/latest), or use Scoop:

```powershell
scoop bucket add mcp-tool-shop https://github.com/mcp-tool-shop-org/scoop-bucket
scoop install commandui
```

`winget install mcp-tool-shop.CommandUI` installs that same public MSI.

## First commands

Open CommandUI. Press Ctrl+J, leave Command selected, type `dir`, and press Enter. You get one result sentence.

Press Ctrl+Shift+A for Ask. There is no Ctrl+2. Type what you want and press Enter. Read the draft. Choose Run Plan, edit it first, or Reject.

## History and workflows

Ctrl+H opens history outside the terminal. Ctrl+Shift+H opens it from the terminal.

Ctrl+Shift+W opens workflows. An empty list has a New workflow button.

## Easy mistakes

- A sentence typed in Command is not a request until you accept the offer to Ask.
- Ask drafts one command. It does not chat.
- Typing in the terminal itself goes straight to the shell and is not listed in History.
