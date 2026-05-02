# Upstream Sync Documentation Alignment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Align user and maintainer docs with the latest completed upstream sync's Linux Computer Use and build-orchestration changes.

**Architecture:** This is a docs-only update. Keep behavior unchanged, update user-facing docs for current Linux Computer Use opt-in semantics, and update maintainer docs so future upstream-sync reviewers preserve the same fork contracts.

**Tech Stack:** Markdown documentation, shell verification commands, GitHub PR workflow.

---

## File Map

- Create: `docs/superpowers/plans/2026-05-02-upstream-sync-doc-alignment.md`
- Modify: `README.md`
- Modify: `docs/usage/build-and-run.md`
- Modify: `docs/maintainers/package-runtime-maintenance.md`
- Modify: `docs/maintainers/fork-divergences.md`
- Do not modify: `CHANGELOG.md`, build scripts, workflows, updater code, ASAR patcher code.

## Task 1: Branch And Save Plan

- [ ] Create and switch to a task branch.

```bash
git switch -c docs/upstream-sync-alignment
```

Expected: branch changes from `main` to `docs/upstream-sync-alignment`.

- [ ] Create `docs/superpowers/plans/2026-05-02-upstream-sync-doc-alignment.md` with this plan content.

Expected: plan is tracked in the same PR as the docs changes.

- [ ] Commit only the saved plan if a staged checkpoint is desired.

```bash
git add docs/superpowers/plans/2026-05-02-upstream-sync-doc-alignment.md
git commit -m "docs: add upstream sync alignment plan

Co-authored-by: Codex <noreply@openai.com>"
```

## Task 2: Update User-Facing Linux Computer Use Wording

- [ ] In `README.md`, replace the Linux Computer Use table note with current behavior:

```markdown
| Linux Computer Use | Packaged; UI controls opt-in | Uses upstream Linux Computer Use support with local packaging/manifest compatibility fixes; requires host accessibility/input support. |
```

- [ ] In `README.md`, keep the OpenAI server-gated row as the only table row that covers account, rollout, and policy gates.

- [ ] In `README.md`, revise the final sentence of the Linux Computer Use section to:

```markdown
This local opt-in only controls Linux UI patching in the generated app. It does not bypass OpenAI account policy, server-side availability, or host accessibility and input prerequisites.
```

- [ ] Verify the README no longer implies Linux Computer Use UI requires account-side rollout.

```bash
rg -n "account-side rollout|requires.*rollout|Statsig rollout" README.md
```

Expected: no current-behavior matches in `README.md`.

## Task 3: Add Build Guide Opt-In Instructions

- [ ] In `docs/usage/build-and-run.md`, add this subsection after the `DMG=/path/to/Codex.dmg` example and before `./install.sh --fresh`:

````markdown
### Linux Computer Use UI Opt-In

The Linux Computer Use backend and plugin manifest are packaged by default. The
in-app UI controls are opt-in because they patch upstream UI paths during app
generation.

Enable the UI patches for one build:

```bash
CODEX_LINUX_ENABLE_COMPUTER_USE_UI=1 make build-app
```

To keep the opt-in across updater rebuilds, write the persisted setting read by
the patcher at `${XDG_CONFIG_HOME:-$HOME/.config}/codex-app/settings.json`. This
matters for updater runs because the `systemd --user` service does not inherit
interactive shell environment variables.

```bash
settings_dir="${XDG_CONFIG_HOME:-$HOME/.config}/codex-app"
mkdir -p "$settings_dir"
python3 - "$settings_dir/settings.json" <<'PY'
import json
import os
import pathlib
import sys

path = pathlib.Path(sys.argv[1])
data = {}
if path.exists():
    data = json.loads(path.read_text() or "{}")
data["codex-linux-computer-use-ui-enabled"] = True
tmp = path.with_name(path.name + ".tmp")
tmp.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")
os.replace(tmp, path)
PY
```
````

- [ ] Verify both opt-in mechanisms are documented in the build guide.

```bash
rg -n "CODEX_LINUX_ENABLE_COMPUTER_USE_UI|codex-linux-computer-use-ui-enabled|codex-app/settings.json" docs/usage/build-and-run.md
```

Expected: all three terms are present.

## Task 4: Update Maintainer Runtime Notes

- [ ] In `docs/maintainers/package-runtime-maintenance.md`, extend the ASAR patch list with:

```markdown
- applies the Linux Computer Use plugin manifest gate by default so the packaged
  backend can register on Linux;
- applies the Linux Computer Use UI patches only when
  `CODEX_LINUX_ENABLE_COMPUTER_USE_UI=1` is set at build time or
  `${XDG_CONFIG_HOME:-$HOME/.config}/codex-app/settings.json` contains
  `"codex-linux-computer-use-ui-enabled": true`.
```

- [ ] Keep the validation policy unchanged; docs-only changes do not require `make build-app`.

- [ ] Verify the maintainer doc uses the fork path, not upstream's old identity.

```bash
rg -n "codex-desktop/settings|codex-app/settings.json|CODEX_LINUX_ENABLE_COMPUTER_USE_UI" docs/maintainers/package-runtime-maintenance.md
```

Expected: no `codex-desktop/settings`; expected opt-in terms are present.

## Task 5: Update Fork Divergence Inventory

- [ ] In `docs/maintainers/fork-divergences.md`, update the ASAR/Linux UI section so the fork delta includes the Computer Use plugin/UI patch split.

- [ ] In `docs/maintainers/fork-divergences.md`, rewrite unrelated "limited to"
      wording if needed so the stale-wording verification has no false positive.

- [ ] Replace the stale Computer Use fork-delta paragraph with:

```markdown
**Fork delta:** Upstream's Linux Computer Use backend and bundled plugin remain
part of the packaged app. This fork preserves the `codex-app` package identity,
keeps the plugin manifest pointed at packaged assets, carries local Linux
input/window-targeting hardening where needed, and documents the local opt-in for
Computer Use UI patching without claiming that local installation changes
OpenAI account policy or server-side availability.
```

- [ ] Replace the stale "Why it matters" paragraph with:

```markdown
**Why it matters:** The package can stage local Computer Use support and register
the backend on Linux, but useful operation still depends on host accessibility,
screenshot, and input prerequisites. Local UI opt-in controls fork-side patching
only; it is not a server-side entitlement change.
```

- [ ] Expand current paths to include the active Computer Use and patching surfaces:

```markdown
**Current paths:** `computer-use-linux/src/`, `plugins/openai-bundled/plugins/computer-use/`,
`scripts/patch-linux-window-ui.js`, `scripts/patch-linux-window-ui.test.js`,
`scripts/lib/package-common.sh`, `launcher/start.sh.template`, `README.md`,
`docs/usage/build-and-run.md`, `CHANGELOG.md`.
```

- [ ] Verify no current fork-divergence wording still says the delta is limited to the old small set.

```bash
rg -n "limited to|account-side rollout|Statsig rollout" docs/maintainers/fork-divergences.md
```

Expected: no stale current-behavior matches.

## Task 6: Verify And Commit

- [ ] Run whitespace verification.

```bash
git diff --check
```

Expected: no output.

- [ ] Run stale wording verification.

```bash
rg -n "codex-desktop/settings|account-side rollout|Statsig rollout|requires.*rollout" README.md docs/usage/build-and-run.md docs/maintainers/package-runtime-maintenance.md docs/maintainers/fork-divergences.md
```

Expected: no current-behavior stale wording. Historical `CHANGELOG.md` is intentionally excluded.

- [ ] Run opt-in coverage verification.

```bash
rg -n "CODEX_LINUX_ENABLE_COMPUTER_USE_UI|codex-linux-computer-use-ui-enabled|codex-app/settings.json" README.md docs/usage/build-and-run.md docs/maintainers/package-runtime-maintenance.md docs/maintainers/fork-divergences.md
```

Expected: README, build guide, and maintainer docs all mention the relevant opt-in mechanism or setting.

- [ ] Review the diff manually.

```bash
git diff -- README.md docs/usage/build-and-run.md docs/maintainers/package-runtime-maintenance.md docs/maintainers/fork-divergences.md docs/superpowers/plans/2026-05-02-upstream-sync-doc-alignment.md
```

Expected: docs only; no generated artifacts or build scripts changed.

- [ ] Commit.

```bash
git add README.md docs/usage/build-and-run.md docs/maintainers/package-runtime-maintenance.md docs/maintainers/fork-divergences.md docs/superpowers/plans/2026-05-02-upstream-sync-doc-alignment.md
git commit -m "docs: align upstream sync documentation

Co-authored-by: Codex <noreply@openai.com>"
```

## Task 7: Open PR

- [ ] Push and create a draft PR.

```bash
git push -u origin docs/upstream-sync-alignment
gh pr create --draft --title "docs: align upstream sync documentation" --body-file /tmp/codex-app-linux-upstream-sync-docs-pr.md
```

- [ ] Use this PR body:

```markdown
## Summary
- aligns Linux Computer Use docs with the latest completed upstream sync range `f6b99eb..5aec7d5`
- documents local UI opt-in behavior in the build guide
- refreshes maintainer runtime and fork-divergence notes for the Computer Use patch split

## Verification
- `git diff --check`
- `rg -n "codex-desktop/settings|account-side rollout|Statsig rollout|requires.*rollout" README.md docs/usage/build-and-run.md docs/maintainers/package-runtime-maintenance.md docs/maintainers/fork-divergences.md`
- `rg -n "CODEX_LINUX_ENABLE_COMPUTER_USE_UI|codex-linux-computer-use-ui-enabled|codex-app/settings.json" README.md docs/usage/build-and-run.md docs/maintainers/package-runtime-maintenance.md docs/maintainers/fork-divergences.md`

## Notes
Docs-only change; no app generation, package build, or updater validation required.
```

- [ ] Mark ready only after the verification commands pass.

## Assumptions

- The implementation is docs-only.
- `CHANGELOG.md` remains historical and is not edited for old release entries.
- Commits after upstream baseline `5aec7d5` are outside this task.
- No build-orchestration change is warranted; current `upstream-build-app` coverage already reflects the fork's local review fixes.
