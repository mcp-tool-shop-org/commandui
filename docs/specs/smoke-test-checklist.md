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
- [ ] Ctrl+H opens history drawer
- [ ] Ctrl+W opens workflow drawer
- [ ] Ctrl+M opens memory drawer
- [ ] Ctrl+, opens settings drawer
- [ ] Esc closes all drawers
- [ ] Ctrl+1 switches to Command mode
- [ ] Ctrl+2 switches to Ask mode
- [ ] Classic mode hides plan panel when empty
- [ ] Guided mode always shows plan panel
- [ ] Reduced clutter hides suggestion panel + markers
