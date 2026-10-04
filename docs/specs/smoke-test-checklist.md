# Smoke Test Checklist

## Boot
- [ ] App launches without errors
- [ ] Session created automatically
- [ ] Terminal shows shell output
- [ ] Header shows session label + cwd

## Raw Shell
- [ ] Type command in composer (Command mode) → executes in shell
- [ ] Output appears in terminal
- [ ] History item created with status success/failure
- [ ] Direct terminal typing works (keystrokes go to PTY)

## Semantic Flow
- [ ] Switch to Ask mode
- [ ] Submit natural language intent
- [ ] Plan appears in right panel
- [ ] Edit command in plan panel
- [ ] Approve → executes edited command
- [ ] Reject → history marked rejected
- [ ] Save Workflow → workflow stored

## Persistence
- [ ] Close and reopen app
- [ ] History items restored
- [ ] Settings restored
- [ ] Workflows restored

## Memory
- [ ] Edit a semantic command → suggestion appears
- [ ] Accept suggestion → memory item created
- [ ] Dismiss suggestion → removed from list
- [ ] Memory drawer shows all items
- [ ] Delete memory item works

## Accessibility + UX
- [ ] Ctrl+H opens history outside the terminal. Ctrl+Shift+H opens it from the terminal
- [ ] Ctrl+Shift+W opens workflows. Ctrl+W closes the session in the desktop app
- [ ] Ctrl+M opens memory outside the terminal
- [ ] Ctrl+, opens settings outside the terminal
- [ ] Escape closes the top dialog and does not reject a plan
- [ ] Ctrl+Shift+A switches Command and Ask. Ctrl+1 switches sessions
- [ ] Classic hides the plan until there is one
- [ ] Guided keeps the plan column open
- [ ] Text size 200% sets the window scale to 2. There is no Reduced clutter setting
