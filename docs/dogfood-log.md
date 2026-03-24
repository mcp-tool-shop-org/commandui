# CommandUI Console — Dogfood Log

Phase 7E smoke testing. Run real programs through Console, log friction.

## Test matrix

### Line-oriented programs

| # | Scenario | Action | Expected | Actual | Severity |
|---|----------|--------|----------|--------|----------|
| 1 | Python REPL | `python -i` in shell | Interactive REPL, prompt visible, input echoed | | |
| 2 | Node REPL | `node` in shell | Interactive REPL, `.exit` works | | |
| 3 | Interactive git | `git log --oneline` | Paged output, scroll works | | |

### Raw Play (fullscreen)

| # | Scenario | Action | Expected | Actual | Severity |
|---|----------|--------|----------|--------|----------|
| 4 | htop / btop | ^G then run `htop` | Full terminal, resize works, ^\ exits | | |
| 5 | vim / nano | ^G then `vim test.txt` | Editor works, saves, ^\ exits | | |
| 6 | TUI game | ^G then roguelike/game | Game renders, input works, ^\ exits | | |

### Multi-session

| # | Scenario | Action | Expected | Actual | Severity |
|---|----------|--------|----------|--------|----------|
| 7 | Two sessions | ^N to create, ^] to switch | Both sessions independent, unread markers | | |
| 8 | Cross-session proposal | ^T Ask on s1, switch to s2, approve | Executes on s1, not s2 | | |

### Onboarding

| # | Scenario | Action | Expected | Actual | Severity |
|---|----------|--------|----------|--------|----------|
| 9 | First launch | Start Console | Welcome banner visible with key hints | | |
| 10 | Help overlay | ^H in shell mode | Help overlay appears, Esc dismisses | | |
| 11 | Raw Play hint | ^G to enter | Entry message visible before game starts | | |

## Severity key

- **defect** — broken behavior, must fix before release
- **friction** — works but confusing or awkward, should fix
- **wish** — nice-to-have, not blocking

## Findings summary

_Fill in after testing._
