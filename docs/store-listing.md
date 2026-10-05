# Store listing (Partner Center, product 9NTN1GFQJ91M)

The listing text for the 1.0.2 update, used for both English and English
(United States). It is entered into the Submission 2 draft. The draft is
submitted only after the accessibility release gate passes (Narrator, NVDA,
two contrast themes, and three people). "Tested for accessibility" stays
unticked until then.

Every line below is backed by the app and its tests. Do not add a claim that
a screen reader, contrast theme, or group of people has tested this build
until those receipts exist.

## Name

CommandUI

## Short description

An accessible Windows shell. Every result in a plain sentence, ask for commands in plain words, and nothing runs until you approve it.

## Description

CommandUI is a Windows shell for people the terminal shuts out: people who use a screen reader or no mouse, people with low vision, people who find terminal output hard to follow, and beginners.

Every command ends with one plain sentence that says whether it worked, and what to do if it did not. A failure offers Ask how to fix it and Run again.

Ask lets you describe what you want in plain words. CommandUI drafts a command, explains it, and shows its risk in words. Nothing runs until you choose Run Plan. A command that deletes files or needs higher permissions also waits until you type the folder name.

Built for the keyboard and for screen readers:
• Results and errors are announced once, without moving your focus.
• Output lists each command as plain text, one region per command.
• F1 opens keyboard help. Ctrl+Shift+R jumps to the last result. Ctrl+Shift+A switches between Command and Ask.
• Text from 100% to 200%, panels you can hide, and Windows contrast themes and reduced motion respected.

You still get a real shell, with your own profile and more than one session. Make workflows from the commands you run, search your history, and read or delete what CommandUI has noticed.

Everything stays on your computer. Ask uses a model on this computer through Ollama (qwen2.5:14b). If that model is not installed or not running, CommandUI says so and does not invent a command. CommandUI sends no telemetry.

## What's new in this version

A new design around accessibility. Every result now ends in a plain sentence, with Ask how to fix it and Run again. Results and errors are announced to screen readers, Output shows each command as plain text, and every action has a keyboard path (F1 lists them). Text size goes from 100% to 200%. Workflows can be created and edited, and a delete can be undone. PowerShell sessions start cleanly in your home folder. Ask says when its local model is not ready instead of guessing. New icon.

## Product features

1. Every result in a plain sentence, with Ask how to fix it and Run again
2. Ask for a command in plain words; nothing runs until you approve it
3. Results and errors announced to screen readers
4. Output view: each command's output as plain text, one region per command
5. Every action by keyboard; F1 lists the shortcuts
6. Text size from 100% to 200%
7. Follows Windows contrast themes and reduced motion
8. Extra confirmation for commands that delete files or need higher permissions
9. Workflows you can create, edit, run, and undo
10. A real shell: PowerShell, cmd, or Git Bash, with your own profile
11. Local-first: a model on your computer through Ollama, and no telemetry

## Search terms

accessible terminal, screen reader, command line, PowerShell, shell, keyboard, Ollama

## Copyright and developer

Copyright 2026 mcp-tool-shop. MIT License. Developed by mcp-tool-shop.

## Images

- Screenshots: from the packaged app of the build being submitted, in the demo folder `E:\Projects\acme-api`, never a home folder (command output prints real paths). At least 1366×768.
- Store logos and display images: from the new logo (brand repo `logos/commandui/`).
