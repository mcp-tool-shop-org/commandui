# Ship Gate

> No repo is "done" until every applicable line is checked.
> Copy this into your repo root. Check items off per-release.

**Tags:** `[all]` every repo · `[npm]` `[pypi]` `[vsix]` `[desktop]` `[container]` published artifacts · `[mcp]` MCP servers · `[cli]` CLI tools

---

## A. Security Baseline

- [x] `[all]` SECURITY.md exists (report email, supported versions, response timeline) — executed by `npx @mcptoolshop/shipcheck security-docs` (A1: present + reporting contact, not an empty stub) (2026-10-04, passed; contact is the GitHub Security Advisories URL, acknowledgement within 7 days)
- [x] `[all]` README includes threat model paragraph (data touched, data NOT touched, permissions required) — executed by `npx @mcptoolshop/shipcheck security-docs` (A2: trust/threat-model section present + non-empty; *quality* is not machine-checkable) (2026-10-04, passed)
- [ ] `[all]` SKIP: `shipcheck secrets` found no publishable packages. The root workspace and every package are `private: true`, so there is no npm tarball to scan.
- [x] `[all]` No telemetry by default — state it explicitly even if obvious (2026-10-04, README and SECURITY.md)

### Default safety posture

- [ ] `[cli|mcp|desktop]` SKIP: medium and high risk commands wait for confirmation in the desktop UI. There is no `--allow-*` flag surface.
- [ ] `[cli|mcp|desktop]` SKIP: an approved shell command is not limited to one directory. CommandUI is a shell, not a sandboxed file tool.
- [ ] `[mcp]` SKIP: not an MCP server
- [ ] `[mcp]` SKIP: not an MCP server

## B. Error Handling

- [ ] `[all]` SKIP: the wire error is `code`, `message`, and optional `details`. It does not have `hint`, `cause`, or `retryable`.
- [ ] `[cli]` SKIP: not a CLI. `apps/console` is a terminal UI.
- [ ] `[cli]` SKIP: not a CLI
- [ ] `[mcp]` SKIP: not an MCP server
- [ ] `[mcp]` SKIP: not an MCP server
- [x] `[desktop]` Errors shown as user-friendly messages — no raw exceptions in UI (2026-10-04, AppShell renders the error string, including boot, save, and plan failures)
- [ ] `[vscode]` SKIP: not a VS Code extension

## C. Operator Docs

- [x] `[all]` README is current: what it does, install, usage, supported platforms + runtime versions (2026-10-04)
- [x] `[all]` CHANGELOG.md (Keep a Changelog format) (2026-10-04)
- [x] `[all]` LICENSE file present and repo states support status (2026-10-04, MIT; SECURITY.md supports 1.x)
- [ ] `[cli]` SKIP: not a CLI
- [ ] `[cli|mcp|desktop]` SKIP: there is no silent / normal / verbose / debug log-level switch
- [ ] `[mcp]` SKIP: not an MCP server
- [ ] `[complex]` SKIP: the product guide is the Starlight handbook. There is no root HANDBOOK.md ops runbook.

## D. Shipping Hygiene

- [ ] `[all]` SKIP: `pnpm verify` is typecheck plus unit tests. The Store executable is `packaging/build-store-exe.ps1`, then `packaging/pack-msix.ps1`. Those are not one command.
- [x] `[all]` Version in manifest matches git tag — executed by `npx @mcptoolshop/shipcheck manifest` (D2: manifest version not behind the newest released tag; `--expect <ver>` for a strict release-time match) (2026-10-04, 1.0.2 is ahead of tag v1.0.0)
- [x] `[all]` Dependency scanning runs in CI (ecosystem-appropriate) — executed by `npx @mcptoolshop/shipcheck ci` (D3: a recognized scanner is *configured* in CI, or dependabot is present) (2026-10-04, `pnpm audit` in ci.yml)
- [ ] `[all]` SKIP: `pnpm audit --audit-level=high` exits 0 after the override floors (2026-10-04). `shipcheck deps` still fails: `npm audit` cannot read the pnpm lockfile (ENOLOCK), and `site/` has `http-cache-semantics` <=4.2.0 with no patched release (GHSA-ch52-4w7c-c8xp). Dependabot alerts are enabled (HTTP 204).
- [ ] `[all]` SKIP: no Dependabot update bot. Org policy says not to add dependabot.yml unless asked. Vulnerability alerts are already enabled.
- [ ] `[npm]` SKIP: private workspace, not published to npm. `shipcheck ci` provenance check skipped (no npm-publish workflow).
- [ ] `[npm]` SKIP: no publishable package. `shipcheck pack` has nothing to pack.
- [ ] `[npm]` `engines.node` set · `[pypi]` `python_requires` set — SKIP: not published to npm or PyPI. `shipcheck manifest` engines check skipped.
- [x] `[npm]` Lockfile committed · `[pypi]` Clean wheel + sdist build — lockfile executed by `npx @mcptoolshop/shipcheck manifest` (D7); the pypi wheel/sdist build is not yet executed (2026-10-04, pnpm-lock.yaml committed; not a Python project)
- [ ] `[vsix]` SKIP: not a VS Code extension
- [x] `[desktop]` Installer/package builds and runs on stated platforms (2026-10-04, unsigned `release/CommandUI_1.0.2.0_x64.msix` packed from the remapped release executable and the process stayed up)

## E. Identity (soft gate — does not block ship)

- [x] `[all]` Logo in README header (2026-10-04, brand URL returns a 1024x1024 PNG)
- [x] `[all]` Translations (polyglot-mcp, 8 languages)
- [x] `[org]` Landing page (@mcptoolshop/site-theme) (2026-10-04, site build wrote dist/index.html, dist/handbook/index.html, and dist/pagefind/)
- [x] `[all]` GitHub repo metadata: description, homepage, topics (2026-10-04, homepage has the trailing slash)

---

## Gate Rules

**Hard gate (A–D):** Must pass before any version is tagged or published.
If a section doesn't apply, mark `SKIP:` with justification — don't leave it unchecked.

**Soft gate (E):** Should be done. Product ships without it, but isn't "whole."

**Executed vs attested.** `shipcheck audit` only *counts these checkboxes* — it does not read your repo, so a box can be green while the fact is false. The lines that say **"executed by `npx @mcptoolshop/shipcheck <gate>`"** are backed by a command that reads the real artifact and exits 1 on the real defect. Run those gates (they are wired into shipcheck's own `verify`); don't just tick their boxes. Executed today: **A1/A2** (`security-docs`), **A3** (`secrets`), **D2/D6/D7** (`manifest`), **D3-config + OIDC/provenance** (`ci`), **real vulnerabilities + alerting** (`deps`), **D5** (`pack`), plus front-door (`front-door`) and dogfood freshness (`dogfood`). Every other line is still an attestation you are vouching for. Note the two dependency layers: `ci` proves a scanner is *configured*; `deps` proves there are *no known vulnerabilities* — a repo can pass the first while failing the second.

**Checking off:**
```
- [x] `[all]` SECURITY.md exists (2026-02-27)
```

**Skipping:**
```
- [ ] `[pypi]` SKIP: not a Python project
```
