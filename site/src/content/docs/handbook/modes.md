---
title: Classic vs Guided Mode
description: How Classic and Guided modes change the CommandUI layout without affecting execution behavior.
sidebar:
  order: 12
---

CommandUI has two product modes that change the UI's behavior. Switch between them in settings (`Ctrl+,`).

## Classic mode

The terminal dominates. The plan panel is collapsed when no plan is active. Chrome is minimal.

**Best for:**
- Users who primarily use Command mode
- Experienced terminal users who occasionally use semantic requests
- Anyone who finds the plan panel distracting when idle

**Behavior:**
- Plan panel appears only when a semantic request generates a plan
- Plan panel hides after approval or rejection
- Maximum terminal real estate

## Guided mode

The plan panel is always visible on the right side. When no plan is active, it shows "No semantic plan yet."

**Best for:**
- Users learning the semantic workflow
- Anyone who uses Ask mode frequently
- First-time users exploring what CommandUI can do

**Behavior:**
- Plan panel is always rendered (right column always visible)
- Empty state text when no plan is loaded
- Provides continuous visual reminder that the semantic path exists

## What does NOT change between modes

Both modes have identical:

- Command execution behavior
- Risk confirmation rules
- History recording
- Memory detection
- Keyboard shortcuts
- Workflow management

The mode affects layout only, never behavior.
