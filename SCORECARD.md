# Scorecard

**Repo:** commandui
**Date:** 2026-10-04
**Type tags:** [all] [npm] [desktop]

## Pre-Remediation Assessment

Measured at the start of this treatment. The desktop app, landing page, and handbook already existed. The ship gate file did not.

| Category | Score | Notes |
|----------|-------|-------|
| A. Security | 6/10 | SECURITY.md existed. The README had no threat section the gate would accept. |
| B. Error Handling | 5/10 | The wire error is code, message, and optional details. Desktop errors are sentences. |
| C. Operator Docs | 7/10 | README, CHANGELOG, and LICENSE were present. The handbook overstated the workflow editor. |
| D. Shipping Hygiene | 4/10 | CI had no dependency audit. Version 1.0.2 was ahead of tag v1.0.0. |
| E. Identity (soft) | 7/10 | Logo, landing page, and handbook were live. Translations were stale. |
| **Overall** | **29/50** | |

## Key Gaps

1. A saved workflow lost its step list after restart.
2. `pnpm audit --audit-level=high` failed on patched dev-tool advisories, and CI did not run it.
3. The handbook said the plan panel opens the workflow editor.
4. Translations still describe the old install.

## Remediation Priority

| Priority | Item | Estimated effort |
|----------|------|-----------------|
| 1 | Store workflow steps as stepsJson and restore them | done |
| 2 | Floor the patched dev dependencies and run pnpm audit in CI | done |
| 3 | Correct the handbook and README, then regenerate translations | translations still open |

## Post-Remediation

`npx @mcptoolshop/shipcheck audit` on 2026-10-04: 15 checked, 23 skipped, 1 unchecked. Pass rate 94%. The unchecked line is translations.

`shipcheck security-docs`, `manifest`, and `ci` passed. `secrets` skipped because every package is private. `deps` failed: npm audit cannot read the pnpm lockfile, and site/ has http-cache-semantics 4.2.0 with no patched release. `pnpm audit --audit-level=high` exits 0. Dependabot alerts return HTTP 204. There is no Dependabot update bot.

Identity scan of the tree: CLEAN.

No Codecov upload was added. Line coverage was not remeasured on this tree. The last recorded workspace figure, 94.73%, belongs to commit 60b41f3.
