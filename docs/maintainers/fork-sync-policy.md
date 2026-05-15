# Fork Sync Policy

This is the current procedure for syncing upstream changes into this fork. Use
it with [Fork Divergences](fork-divergences.md), which is the canonical
inventory of local contracts.

The local policy config is `.agents/fork-sync-policy.toml`. It exists for
agents and maintainers; runtime code does not consume it.

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
7. Update the upstream baseline in
   [Fork Divergences](fork-divergences.md) after the sync. The policy config
   points to that canonical inventory instead of duplicating the mutable commit
   hash.
8. Compare upstream README and other user-facing docs against this fork's
   README and usage docs. Classify readme-relevant upstream additions as
   adapted under local contracts, already covered, intentionally omitted, or
   follow-up.
9. Keep a sync ledger in the PR body or a temporary working note until it is
   copied into the PR.
10. Run the required local gates before the first push that contains code
   changes covered by [Local Gates](#local-gates).
11. On the first push of any task branch, create a draft PR in the same
   workflow turn.
12. Use `--repo nisavid/codex-app-linux` on every `gh pr` command in this
   checkout. Do not rely on GitHub CLI's inferred repository; it can target the
   wrong repository in this fork checkout.
13. Keep the PR in draft until local gates pass and the PR body records
   verification evidence. For code-changing branches, the required lifecycle is:
   local gates, first push, draft PR, PR verification notes, ready for review.
14. Inspect GitHub blockers directly. Do not infer merge readiness from summary
   status alone.

## Sync Ledger

Every broad upstream sync needs a ledger with:

- upstream refs fetched and the baseline commit;
- policy files read;
- every divergence area checked;
- upstream README and other user-facing doc changes reviewed;
- readme-relevant additions classified as adapted under local contracts,
  already covered, intentionally omitted, or follow-up;
- baseline update made in [Fork Divergences](fork-divergences.md);
- incoming changes that affect local contracts;
- classification for each affected area: preserved, upstream now implements it,
  obsolete by policy, intentionally changed, or uncertain;
- exact local verification commands and results;
- unresolved uncertainties for maintainer triage.

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
baseline. If the impact is uncertain, list it for maintainer triage instead of
choosing by assumption.

Treat upstream README and usage-doc changes as product facts to review, not as
text to copy wholesale. Pull over facts that affect supported platforms, host
requirements, feature gates, install/update commands, troubleshooting, or
validation, but translate names, paths, service identifiers, package filenames,
and commands to this fork's local contracts.
