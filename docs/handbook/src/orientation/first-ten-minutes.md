# First Ten Minutes

## Launch

CommandUI opens one shell session. The command box is at the bottom, with Command and Ask. Classic hides the plan until there is one. The empty plan says "No plan yet."

## Run a command

1. Press Ctrl+J to reach the command box. From the terminal, use Ctrl+Shift+J.
2. Choose Command. Ctrl+Shift+A switches between Command and Ask. Ctrl+1 switches sessions. It does not change the mode.
3. Type `dir` and press Enter.

The command runs in your shell. The result is one sentence, such as Finished. History records it.

If you type a sentence in Command, CommandUI asks "This looks like a request. Ask CommandUI instead?" and does not run the sentence until you choose.

## Ask for a command

1. Switch to Ask with the Ask button or Ctrl+Shift+A.
2. Type "show me changed files" and press Enter.

If the model is ready, a plan opens. It says what the command does, in words, and the risk in words. You can edit the command, choose Reject, or choose Run Plan.

If the model is not installed, not running, or not downloaded, Ask names which one and the next step. Check again is on that notice. A release build does not draft a stand-in command.

## History

Press Ctrl+H outside the terminal, or Ctrl+Shift+H from the terminal. Each row says where it came from, in words: Typed command, From Ask, or From Ask, practice plan. The status is a word such as Finished or Did not work.

## Settings

Press Ctrl+, outside the terminal, or open Settings from the command palette. Text size runs from 100% to 200%.

## A workflow

Open workflows with Ctrl+Shift+W. An empty list tells you how to fill it and offers New workflow.
