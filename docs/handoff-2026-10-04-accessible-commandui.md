# Handoff: make CommandUI an accessibility tool, for real

Written 2026-10-04 on branch `brand/new-logo` (commit 153060c plus this file).
The owner tried the packaged 1.0.2 and stopped the Store release. His decision
was that CommandUI is an accessibility tool, and that this build is shaped
around the machine rather than the person using it. Two concrete triggers: a
red **FAILURE** badge with no other information, and no visible way to add a
workflow.

> **Superseded in part, 2026-10-05 (owner's decision).** 1.0.2 ships to the
> Store as the functional update, without workstream 6's assistive-technology
> runs. Those runs move to the next update, which is the one that may claim
> tested accessibility. 1.0.2 makes no such claim, and "Tested for
> accessibility" stays unticked.

The 1.0.2 Store submission (Partner Center product 9NTN1GFQJ91M, Submission 2)
stays in draft until the workstreams below marked **release gate** are done and
their checks pass. Nothing in this file is done yet unless it says so.

## The product, stated

CommandUI makes the command line usable by people the terminal shuts out:

- screen-reader users;
- low-vision users;
- keyboard-only users;
- people with cognitive or learning disabilities;
- beginners.

It does this by explaining every result in plain words, by letting the user ask
for a command in plain words, and by never running anything the user has not
seen and approved. Experts keep a real shell (PTY sessions, their own profile)
and lose nothing.

Today none of this is the stated product. README.md:19 and the landing page
(site/src/site-config.ts:5, :15) say "AI-native shell environment" and show
`git` and `pnpm` to developers. The word "accessibility" appears only in
testing checklists. **Workstream 7 rewrites the positioning. Every other
workstream builds what that positioning promises.**

## What is wrong today (audit of 153060c, every item verified in code)

1. **FAILURE says nothing.** `components/TerminalPane.tsx:251-255` renders only
   the status word: 11 px, top right, no exit code, no command, no reason, no
   next step, no announcement. Four different situations all produce this same
   bare word:
   - an ordinary non-zero exit;
   - a made-up exit code 1 when cmd never reported one
     (`UNKNOWN_EXIT_CODE`, session_service.rs:26);
   - the shell dying;
   - a write failure.

   Plain English typed in Command mode (the default) runs as a shell command and
   gets the same badge.
2. **Workflows cannot be created where a user looks.** The Workflows drawer
   only lists, runs and deletes (`WorkflowDrawer.tsx:104`). The three real ways
   to make one are hidden elsewhere:
   - "Save Workflow" on an Ask plan;
   - "Save Workflow" on an expanded History row;
   - a suggestion that appears after the same sequence has run 3 times across
     2 sessions. Turning on "Reduced clutter" hides this route too.

   Other gaps: there is no "New workflow" button, a workflow cannot be edited,
   and the editor cannot add a step. The `projectRoot` prop the handbook
   promises is never read.
3. **A screen reader gets nothing from the terminal.** xterm's
   `screenReaderMode` is never set (`TerminalPane.tsx:137-148`). There is no
   `aria-live` anywhere in the app, so a command finishing or failing is never
   announced. The error box has no `role="alert"`.
4. **Errors leak internals, or say "[object Object]".**
   - `lib/tauriInvoke.ts:33` stringifies the Rust `ApiError` object, and a test
     asserts the "[object Object]" text (`persistInBackground.test.ts:42`).
   - The wrapper adds internal command names: "Command 'terminal_execute' failed".
   - Other messages use internal words: "desynced", "has not reported ready",
     "sync failed", "plan store".
   - Plan safety flags show as raw enum codes such as `DESTRUCTIVE_OPERATION`.
   - Developer messages ship to users: "check that your Tauri backend is
     running", "Run `pnpm tauri:dev`".
5. **Ask pretends.** When Ollama is missing, not running, or lacks
   `qwen2.5:14b`, the planner falls back to the mock silently. The cause only
   goes to stderr (runtime-planner/src/lib.rs:13-25).
   - The mock answers most requests with `echo "mock: placeholder"`, labelled
     low risk and approvable in one click.
   - The only sign is a faint "Mock planner — Ollama not connected" line.
   - History records `ollama` as the source even for mock plans (AppShell.tsx:1295).
   - The model is hard-coded, with no setting.
6. **Drawers are not dialogs.** They have no `role="dialog"`, do not move focus
   in when they open, and do not trap focus. Several can be open at once
   (History stays under Workflows). Escape closes all of them at once. When no
   drawer is open, Escape rejects a pending plan unless focus is in the terminal.
7. **Controls carry no state for assistive tech.**
   - The Command/Ask toggle has no `aria-pressed` and no keyboard shortcut. The
     handbook claims Ctrl+2, but Ctrl+1–9 switch sessions.
   - Session tabs have no tablist semantics, and every close button is named
     just "Close session".
   - The command box and the plan's command field have no label.
   - The command box is disabled while a command runs, so keyboard focus drops.
   - The submit button says "Run" even in Ask, where it only drafts a plan.
8. **Vision and motion.**
   - UI text is 10–12 px in many places, and the terminal is a fixed 14 px.
     A `fontSize` setting exists in the types with no UI behind it.
   - Badge contrast in the light theme is as low as 2.8:1.
   - The terminal stays dark in the light theme.
   - Workflow status dots rely on colour alone.
   - There is no `prefers-reduced-motion`, `forced-colors` or `prefers-contrast`
     handling. Two animations pulse forever.
9. **Settings mislead.**
   - "Reduced clutter" only hides memory suggestions, but the handbook says it
     hides markers that no longer exist.
   - Classic/Guided only toggles one column.
   - The labels are not associated with their selects, and no setting has a
     description.
   - `theme`, `fontSize`, `density`, `autoOpenPlanPanel` and
     `explanationVerbosity` exist in the types with no UI and no code.
10. **Destructive actions** (delete workflow, delete memory) have no confirm and
    no undo. Three `window.confirm` popups are native and unstyled.

## Research grounding

Every source below was checked on 2026-10-04: arXiv and Crossref titles match,
and the Microsoft and W3C pages were reached.

| Source | Finding | What it means here |
|---|---|---|
| Sampath, Merrick, Macvean, *Accessibility of Command Line Interfaces*, CHI 2021, doi:10.1145/3411764.3445544 | For blind and low-vision developers the barrier is unstructured text. Spinners and progress bars are read out as streams of glyphs, and grid tables are worse than plain text. | Give each command its own structured block (command, result, output). Turn progress redraws into quiet numbers. |
| VS Code accessibility docs, code.visualstudio.com/docs/configure/accessibility/accessibility; VS Code shell integration docs | An Accessible View of terminal output, an Accessibility Help dialog, per-event sounds and announcements (e.g. "terminal command failed"), and command navigation driven by shell-integration markers. | CommandUI already emits its own start, end and exit markers (OSC 7733). Build per-command reading and announcements on those, not on scraped output. |
| xterm.js screenReaderMode (issues #1931, #6185) | It exposes visible rows to screen readers, but has no notion of commands, and dictation and on-screen-keyboard input can be dropped. | Turn it on, but make our own accessible view the primary reading surface. Test dictation. |
| MDN, ARIA live regions | `polite` waits and `assertive` interrupts. The region must exist before its content changes, and rapid updates are coalesced. | Use one persistent `role="status"` region for results, debounced to one message per command. Use `role="alert"` only for errors that block the user. |
| Marceau, Fisler, Krishnamurthi, SIGCSE 2011, doi:10.1145/1953163.1953308; Becker et al., ITiCSE-WGR 2019, doi:10.1145/3344429.3372508 | Novices misread error messages because of unfamiliar words and because the message does not say what to do next. Friendlier wording alone has mixed results. | Every failure states what happened, the likely cause and one concrete next step. Raw shell text sits behind "Show details". Test with real users. |
| Buçinca, Malaya, Gajos, CSCW 2021, arXiv:2102.09692; Vasconcelos et al., CSCW 2023, arXiv:2212.06823 | Thinking prompts cut overreliance on AI more than explanations do, but people dislike them. Explanations help only when checking the answer is cheap. | Ask: explain each part of the drafted command and preview its effect, such as which files. Add friction (type the folder name) only for high-risk commands, so ordinary approvals stay one step. |
| Lin et al., NL2Bash, LREC 2018, arXiv:1802.08979; Prather et al. 2023, arXiv:2304.02491; Vaithilingam et al., CHI EA 2022, doi:10.1145/3491101.3519665 | NL-to-shell output is often subtly wrong, and novices accept generated code without understanding it. | Treat every drafted command as unverified. Put the explanation before Approve, and make Edit and Reject as easy as Approve. |
| NN/g, *Designing empty states*; NN/g, *Progressive disclosure*; Li et al., SUGILITE, CHI 2017, doi:10.1145/3025453.3025483 | An empty state should say what the feature is for and how to start. Recording what the user just did, then naming it, makes automation easier to create. | Workflows: "New workflow" in the drawer, and "Save the last N commands as a workflow" from History. Advanced editing stays behind the first save. |
| W3C, WCAG 2.2 (2023), w3.org/TR/WCAG22 | Level A/AA criteria (listed under Workstream 6). | They become the acceptance checks below. |
| W3C COGA Task Force, *Making Content Usable for People with Cognitive and Learning Disabilities* (2021), w3.org/TR/coga-usable | Clear purpose, familiar design, help available, prevent mistakes, plain wording, no time limits. | Confirm with undo for destructive actions, help in the same place everywhere, and no timeouts on decisions. |
| ISO 24495-1:2023, *Plain language* | Content must be relevant, findable, understandable and usable for its readers. | Wording checklist for every message: what happened, why, what to do, no jargon, no blame. |
| Microsoft Learn, *Accessibility in the Store* and the accessibility checklist; Accessibility Insights for Windows | Declare "tested for accessibility" only after Narrator, keyboard, contrast, High Contrast and High DPI testing, with high-priority issues fixed. | **Do not tick that Partner Center box until Workstream 6 has receipts.** |

Gap the research did not fill: there are no terminal-specific studies for
cognitive disabilities or low vision beyond contrast. Treat our choices there as
hypotheses and test them with people (Workstream 6).

## Rules for this work

These rules are what keep the result from being placeholder theater.

1. **No promise without code and a test.** A sentence in the UI, the welcome
   screen, the README or the handbook that describes behaviour must point at
   code and a test that proves it. Delete promises you cannot back.
2. **Every setting changes something you can observe, and a test proves it.**
   Remove or rename settings that do not.
3. **Every empty state names the way to fill it, and offers that way as a button.**
4. **Every failure or error says three things:** what happened, the likely cause,
   and what to do next. Show these as text, not colour, and announce them.
5. **No internal word in user-visible text.** A test scans user-visible strings
   against a banned list. Seed it with: semantic, desync/desynced, booting,
   plumbing, marker, PTY, exec, mock (outside the planner-status screen),
   Tauri, backend, database, raw enum codes, `[workflow:`, `[plan]`, and
   `Command '…' failed`. Each word is either replaced or explained.
6. **Assistive technology is tested, not assumed.** Automated checks (axe in
   Vitest, Accessibility Insights) are necessary but not enough. Each release
   gate includes a scripted Narrator and NVDA run with a receipt.
7. **Do not lower the bar to pass.** Do not exclude files from coverage, mark a
   test skipped, or soften an acceptance check without the owner's OK in
   writing.

## Workstreams

Do them in order. Each one ends with its checks green and a short note in the
PR. Workstreams 0–6 are the release gate.

### 0. Honesty fixes (small, first)

- `tauriInvoke` turns `{code, message, details}` into a readable message and
  keeps `code` for matching. Fix the test that asserts "[object Object]".
- Matching by error text (`isMissingIdError`, `/has exited/`) moves to the
  error code.
- History records the planner source that actually produced the plan
  (`ollama` or `mock`), as the backend returns it.
- Escape never rejects a plan. Reject stays on its button and on the R key.
- Remove developer messages from the shipped app ("pnpm tauri:dev", "Tauri
  backend"), or replace them with something a user can act on.
- Delete workflow and delete memory: confirm in an in-app dialog, then allow
  Undo for 10 seconds. Replace the three `window.confirm` popups with the
  in-app dialog.

**Checks:**
- A rejected backend call renders the Rust message text. A test covers each
  error code path.
- A mock plan is recorded as `mock`.
- Escape with a plan open and no drawer leaves the plan in place.
- Delete, then Undo, restores the item.

### 1. Every command result explains itself

- Replace the badge with a **result line** under each command:
  - Success: "Finished. 24 lines of output."
  - Failure: "Did not work (exit code 1)", followed by a plain-language reason
    when one is known, and the actions **Show output**, **Ask how to fix it**
    and **Run again**.
  - Running: "Running… (Stop)".
- Known causes, each with its own wording and test:
  - command not found;
  - access denied;
  - path not found;
  - a command typed in Command mode that reads like a sentence. Offer
    "This looks like a request. Ask CommandUI instead?", which moves the text
    to Ask.
  - exit code unknown: say "CommandUI could not tell whether this worked"
    rather than inventing exit 1;
  - shell exited;
  - input not accepted.
- **Ask how to fix it** sends the command, exit code and the last N lines of
  output to Ask as context.
- The same text goes once to the polite status region (Workstream 2).

**Checks:**
- Tests for each cause render the right words and actions.
- No path can render the word FAILURE alone.
- A sentence typed in Command mode produces the Ask offer.

### 2. Screen reader and keyboard

- Turn on xterm `screenReaderMode`. Test typing, dictation (Windows Voice
  Typing) and paste with it on.
- Add an **output view** (a list of command blocks: the command, the result line,
  then its output as plain text). Build it from the existing OSC 7733 markers.
  It opens with a shortcut and from a visible button. Progress redraws collapse
  to their last state.
- **Announcements:** one persistent `role="status"` region announces each
  command's result line once, debounced. Errors that block the user go to a
  `role="alert"` region. Approval prompts move focus to the plan, and the plan
  is announced.
- **Drawers and the palette become modal dialogs:**
  - `role="dialog"`, `aria-modal`, and a name;
  - focus moves in when they open and is trapped while open;
  - only one is open at a time;
  - Escape closes the top one and returns focus to where it was.

  Reuse the WelcomeScreen pattern.
- **Controls:**
  - The Command/Ask toggle becomes a labelled group with `aria-pressed`, plus a
    shortcut. Pick one that does not clash with Ctrl+1–9, and correct the
    handbook's Ctrl+2.
  - Session tabs become a `tablist` with `aria-selected`. Each close button
    names its session.
  - The command box and the plan's command field get labels.
  - The command box stays focusable while a command runs (read-only, not
    disabled).
  - The submit button reads **Draft plan** in Ask.
- **Accessibility help:** a dialog (F1) lists the shortcuts and explains the
  output view and the announcements. Its entry point is in the same place on
  every screen.
- **Terminal escape:** a documented key moves focus out of the terminal, since
  the terminal keeps Tab for itself (WCAG 2.1.2).

**Checks:**
- axe (vitest-axe) passes on AppShell, every drawer and the welcome screen,
  with zero violations.
- The five core tasks (Workstream 6) can be done keyboard-only.
- Narrator and NVDA scripts announce the result of a failing command and of a
  successful one, each once.

### 3. Ask is honest and safe

- **Planner status** in Settings and on the plan panel. It says which of these
  is true, in plain words:
  - Ollama not installed;
  - Ollama not running;
  - model not downloaded (naming the model);
  - ready.

  Each has a fix step with a link, and a **Check again** button. The Rust
  planner returns the reason instead of only printing it to stderr.
- The model and the endpoint become settings (default `qwen2.5:14b`,
  `http://localhost:11434`).
- **The mock planner never acts like a real one** in a release build. If no
  model is ready, Ask says so and offers the fix. It does not draft
  `echo "mock: placeholder"`. Keep the mock for tests and browser preview, and
  label it there.
- **Plans explain before Approve:**
  - one plain sentence on what the command does;
  - a part-by-part explanation;
  - a preview of what it will touch, where that can be known (for example a
    file list for a delete);
  - risk shown in words ("Deletes files: cannot be undone").
- Safety flags are written out in plain words.
- **Friction only for high risk.** For a high-risk command the user types a
  confirmation (for example the folder name). Low-risk commands approve in one
  step.

**Checks:**
- A test for each planner state shows its text and fix.
- The release build with no Ollama never shows a runnable placeholder plan.
- A high-risk plan cannot be approved without the typed confirmation.
- Every flag has a plain-language string.

### 4. Workflows a person can make

- **New workflow** in the Workflows drawer opens the editor with a name field
  and Add step, Edit step, Move and Remove.
- **Edit** an existing workflow. If the handbook keeps promising project scope,
  implement it; otherwise remove the promise.
- From History: select rows, then choose **Save as workflow**. This also covers
  "save the last N commands".
- The empty state: "Workflows are saved lists of commands you can run again
  with one click. Make one here, or save commands from History." It has a
  **New workflow** button.
- Pattern suggestions are no longer hidden by "Reduced clutter". The setting
  is renamed or removed (Workstream 5).
- Show dates as relative text ("2 days ago"), with the full date on hover and
  for screen readers.

**Checks:**
- Create, edit, run and delete (with undo) a workflow keyboard-only, with tests.
- The empty state renders its button, and the button opens the editor.

### 5. Vision, motion and plain language

- **Text size setting** wired to the existing `fontSize` type. It scales the UI
  and the terminal (from 100 % to 200 %) with no clipping (WCAG 1.4.4).
- **Contrast:** at least 4.5:1 for text and 3:1 for controls and focus rings,
  in both themes. That includes the result line and the terminal palette, and
  the terminal follows the light theme. A test reads the CSS tokens and asserts
  the ratios.
- **`forced-colors`:** borders, focus and state stay visible in Windows
  contrast themes. **`prefers-reduced-motion`:** stops the pulses and the
  cursor blink.
- **No colour-only state:** workflow dots and history chips carry a text or
  icon label.
- Pointer targets are at least 24×24 px (WCAG 2.5.8).
- **Settings:**
  - every setting gets a one-line description;
  - labels are associated with their controls;
  - "Reduced clutter" is removed, or renamed to what it does;
  - Classic/Guided is explained or merged;
  - dead types are removed.
- **Microcopy pass:** every user-visible string is rewritten to the plain-language
  checklist (what happened, why, what to do, no jargon, no blame). The
  banned-words test from the rules enforces it.

**Checks:**
- The contrast token test passes.
- At 200 % scale there is no clipping (screenshot test or manual receipt).
- With forced-colors and reduced-motion emulated, the UI stays usable (Playwright
  or a manual receipt).
- The banned-words test passes.

### 6. Prove it (release gate)

- **Automated:**
  - axe has zero violations on every screen;
  - Accessibility Insights for Windows (FastPass) on the packaged app has zero
    failures;
  - all CommandUI CI is green, coverage gates included.
- **Scripted manual runs, each with a dated receipt** in `docs/a11y-receipts/`:
  - keyboard only;
  - Narrator;
  - NVDA;
  - Magnifier at 200 %;
  - High Contrast;
  - 200 % display scaling;
  - Windows Voice Typing in the command box.

  Each run covers the **five core tasks:**
  1. run a command and hear or see whether it worked;
  2. recover from a failed command using **Ask how to fix it**;
  3. ask for a command in plain words, review it and approve it;
  4. create a workflow and run it;
  5. change the text size.
- **People:** at least 3 people from the target groups do the five tasks with no
  help. The owner recruits them. Record what blocked them and fix it before the
  release.
- Only then tick "tested for accessibility" in Partner Center.

### 7. Say what it is (after 0–6)

- Rewrite README, landing page and handbook around the product statement. Cut
  every claim that Workstreams 0–6 did not prove, such as Ctrl+2, the
  "Reduced clutter" markers and project scope.
- New Store listing text and screenshots from the packaged app, using a neutral
  demo folder and never a real home folder.
- Translations last, before the tag (studio release order).

## What is already done on this branch (153060c)

- New logo across MSIX assets, Tauri icons and the brand repo (brand PR #95).
  WACK passes.
- PowerShell starts with no bootstrap echo, proven by a live test that fails on
  the old code.
- Sessions start at home, not System32.
- Paths show the home folder as `~` and long paths are shortened.
- A welcome screen (modal, can be turned off, 9 tests). **Its text must be
  re-checked against Workstreams 1–4 once they land:** it promises one-click
  workflows, which Workstream 4 has to make true.
- Separately, the coverage gates are on PR #9 (Rust and TypeScript at 90 %).

## Size and split

This is a feature pass, not a polish pass. It has four lanes that touch
different files, so they can run in parallel:
- **A:** Workstreams 0 and 1, results and errors (TerminalPane, AppShell
  result wiring, Rust exit reasons).
- **B:** Workstream 2, screen reader and dialogs (drawers, palette, live
  regions, output view).
- **C:** Workstream 3, Ask and the planner (runtime-planner, PlanPanel,
  Settings).
- **D:** Workstreams 4 and 5, workflows, visual and settings.

Workstream 6 runs after all four lanes have merged. The owner takes part
because of the human tests.

## Standards compliance

| Standard | Score | Evidence or remediation |
|---|---|---|
| PIN_PER_STEP | 1 | Pins the base commit (153060c) and the planner model; lanes do not pin models. Remediation: the swarm dispatch records model and commit per lane (the owner's session at kickoff). |
| ANDON_AUTHORITY | 2 | Every workstream ends on checks. Rule 7 forbids lowering the bar. The release gate stops the Store submission. |
| NAMED_COMPENSATORS | 2 | No irreversible step until Workstream 7. The Store submission stays in draft (deleting the draft is its compensator). The release itself follows docs/release-order.md in Publisher. |
| DECOMPOSE_BY_SECRETS | 2 | Lanes split by the files and concerns that change together. |
| UNCERTAINTY_GATED_HUMANS | 2 | The owner decides the product statement, takes part in the people test, and approves any rule exception in writing. |
| EXTERNAL_VERIFIER | n/a | No specialized claims. The accessibility proof is human testing (Workstream 6), not a model verdict. |
