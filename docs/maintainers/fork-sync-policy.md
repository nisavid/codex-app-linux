# Fork Sync Policy

This is the current procedure for syncing changes from the Linux-port upstream
into this fork. In this document, `upstream` means the `upstream` remote for
`ilysenko/codex-desktop-linux` unless a sentence names another surface. Use this
procedure with [Fork Divergences](fork-divergences.md), the canonical inventory
of local contracts and terminology.

The local policy config is `.agents/fork-sync-policy.toml`. It exists for
agents and maintainers; runtime code does not consume it.

<!--
Future refactor note: this file still contains generic fork-sync procedure
because repo-local parameters are interleaved with the workflow. When revisiting
it, identify the general rules and the localization parameters they need, such
as remotes, target repository, required merge method, local gates, ledger
fields, rename maps, and issue or backlog destinations, then migrate the common
behavior into the user-global `syncing-forks-with-upstream` skill.
-->

## Required Workflow

1. Create a task branch. `main` is protected.
2. Fetch `origin` and `upstream`.
3. Read [Fork Divergences](fork-divergences.md),
   `.agents/fork-sync-policy.toml`, and this document before resolving
   conflicts.
4. Use the user-global `syncing-forks-with-upstream` skill before choosing a
   merge method or pushing a sync branch. If that external skill is unavailable,
   continue from this document and record the missing-skill fallback in the sync
   ledger.
5. Preserve upstream commit identity. If a PR is required, merge the sync with a
   normal merge commit, not a rebase or squash merge.
6. Preserve this fork's intentional contracts unless the PR intentionally
   changes policy.
7. Update the upstream baseline in [Fork Divergences](fork-divergences.md) after
   the sync. The policy config points to that canonical inventory instead of
   duplicating the mutable commit hash.
8. Compare upstream user-facing docs against this fork's README and usage docs.
   Classify relevant additions as adapted under local contracts, already
   covered, intentionally omitted, or follow-up.
9. Check [Renamed Path Reconciliation](#renamed-path-reconciliation) before
   resolving missing-file, modify/delete, rename/delete, or add/add conflicts.
10. Close any reusable policy gap found during the sync. If the sync reveals a
   hazard that future agents could miss, update the narrowest durable policy
   surface before handoff.
11. Create or update an in-tree sync ledger entry under
   [Fork Sync Ledger](fork-sync-ledger/) before closeout. The PR body may carry
   a concise summary, but the tracked ledger entry is the durable source.
12. Run the required local gates before the first push that contains code
   changes covered by [Local Gates](#local-gates).
13. On the first push of any task branch, create a draft PR in the same
   workflow turn.
14. Use `--repo nisavid/codex-app-linux` on every `gh pr` command in this
   checkout. Do not rely on GitHub CLI's inferred repository; it can target the
   wrong repository in this fork checkout.
15. Keep the PR in draft until local gates pass and the PR body records
   verification evidence. For code-changing branches, the required lifecycle is:
   local gates, first push, draft PR, PR verification notes, ready for review.
16. Inspect GitHub blockers directly. Do not infer merge readiness from summary
   status alone.

## Sync Ledger

Every broad upstream sync needs a tracked ledger entry under
[Fork Sync Ledger](fork-sync-ledger/) with:

- upstream refs fetched and the baseline commit;
- policy files read;
- every divergence area checked;
- upstream user-facing doc changes reviewed;
- readme-relevant additions classified as adapted under local contracts,
  already covered, intentionally omitted, or follow-up;
- renamed-path checks completed, including any manual old-path to current-path
  reconciliations;
- policy gaps found and codified, or a note that no reusable gap was found;
- baseline update made in [Fork Divergences](fork-divergences.md);
- incoming changes that affect local contracts;
- classification for each affected area: preserved, upstream now implements it,
  obsolete by policy, intentionally changed, or uncertain;
- exact local verification commands and results;
- special-handling highlights that future maintainers may need to review;
- follow-up decisions for each special-handling item, including links to
  existing issues, newly created issues, or a note that no issue is warranted;
- unresolved uncertainties escalated to the operator, or linked to a durable,
  discoverable follow-up when escalation is unavailable.

Do not push while the ledger has unchecked divergence areas, untriaged
uncertainty, or missing required local gates.

## Local Gates

Before pushing changes that affect the generated app, installer, ASAR patcher,
package builders, package payload, updater rebuild flow, or bundled runtime
helpers:

1. Refresh `Codex.dmg`, or verify the cached DMG was refreshed within the last
   24 hours.
2. Run `make build-app` or `./install.sh` from current sources.
3. If package contents changed, run the relevant package builder and inspect
   package metadata plus file listings.
4. If release workflow changed, run the relevant release gate.
5. Record exact commands and results in PR verification notes before marking the
   PR ready for review.

CI is secondary evidence for these surfaces. It does not replace the local
build gate.

## Contract Review

Review incoming changes against every area in
[Fork Divergences](fork-divergences.md#divergence-inventory). In particular,
protect local product names, package names, install paths, XDG/FHS layout,
package versioning from the OpenAI DMG bundle, updater privilege boundaries,
package payload shape, and security gates.

If an upstream change appears to implement the same behavior, update the
divergence inventory to describe the current diff against the synced upstream
baseline. If the impact is uncertain, escalate to the operator when the session
allows. Only defer the decision when escalation is unavailable or the operator
requested an uninterrupted run; in that case, record a durable, discoverable
follow-up where the escalation would have happened. The PR body can link to that
follow-up, but it is not sufficient by itself.

Treat upstream README and usage-doc changes as product facts to review, not as
text to copy wholesale. Pull over facts that affect supported platforms, host
requirements, feature gates, install/update commands, troubleshooting, or
validation, but translate names, paths, service identifiers, package filenames,
and commands to this fork's local contracts.

## Policy Gap Closeout

Treat discovered repeatable sync hazards as part of the sync, not as optional
retrospective notes. If a conflict, missed change, review comment, local gate,
or manual reconciliation exposes a rule future agents need, update the
narrowest durable surface before handoff:

- `docs/maintainers/fork-divergences.md` for repo-specific contracts, rename
  maps, baselines, and divergence checks;
- `.agents/fork-sync-policy.toml` for machine-readable repo-local policy flags
  and pointers;
- this document for repo-local sync workflow;
- `AGENTS.md` for rules that must be preloaded before an agent can choose a
  triggered workflow;
- the user-global `syncing-forks-with-upstream` skill for behavior that applies
  across maintained forks;
- tests or scripts for repeatable mechanical checks.

If the right owner is uncertain, escalate to the operator when the session
allows. Only defer the decision when escalation is unavailable or the operator
requested an uninterrupted run; in that case, record a durable, discoverable
follow-up where the escalation would have happened, and keep the safest local
guard that prevents dropped upstream changes, history replay, contract drift,
or missing verification.

## Renamed Path Reconciliation

Git's merge strategy normally performs rename detection, but it is similarity
based and can still surface an upstream edit as a missing old path,
modify/delete conflict, rename/delete conflict, or resurrected file. Treat those
states as reconciliation work, not permission to drop the upstream change.

Before resolving sync conflicts:

1. Review the current rename map in
   [Fork Divergences](fork-divergences.md#current-local-rename-and-compatibility-map).
2. Inspect the merge with rename-aware commands:

   ```bash
   git status --renames
   git diff --name-status --find-renames HEAD...MERGE_HEAD
   ```

3. For each upstream change to an old path, apply the equivalent change to the
   current local path. In an active merge, this pattern keeps the old-path diff
   visible while you reconcile:

   ```bash
   base="$(git merge-base HEAD MERGE_HEAD)"
   git diff "$base" MERGE_HEAD -- old/path
   git show MERGE_HEAD:old/path
   ```

   Replace `MERGE_HEAD` with the upstream ref when inspecting before starting a
   merge.

4. Remove resurrected old paths only after their incoming changes are ported or
   intentionally rejected:

   ```bash
   git rm old/path
   git add current/path
   ```

5. Record each manual reconciliation or intentional omission in the sync
   ledger, including the old path, current path, and verification run.

If Git automatically maps the rename, still confirm the resulting current file
contains the incoming upstream change and that the old path remains absent
unless compatibility requires it.
