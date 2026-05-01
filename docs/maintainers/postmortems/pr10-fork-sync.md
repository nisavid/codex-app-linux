# PR #10 Fork Sync Post-Mortem

PR #10 synced upstream `main` into this fork and preserved upstream commit
identity, but it repeatedly regressed intentional fork behavior. The repair
work after the merge established the current fork-sync policy.

## What Failed

- The sync optimized for upstream commit identity without giving equal weight
  to local fork contracts.
- Local names, install paths, package versioning, docs, and policy surfaces
  were not treated as protected contracts at the start of conflict resolution.
- Upstream-origin features introduced by the sync were sometimes described as
  fork-origin features before checking the actual diff against the upstream
  baseline.
- README and maintainer docs were allowed to drift from the pre-sync fork
  state, which made later repair harder.
- Local build evidence was not treated as a push gate early enough.
- DMG freshness was not enforced until the maintainer called it out.
- GitHub merge blockers and CodeQL details were inferred too coarsely before
  direct inspection.
- Uncertain path and layout conflicts were resolved too confidently instead of
  being listed for maintainer triage.

## Root Causes

- The repo had a divergence inventory, but the sync workflow did not make it a
  mandatory pre-conflict input.
- The global fork-sync skill focused on preserving upstream commit objects and
  did not cover fork-contract preservation.
- The repo lacked a compact policy config that an agent could discover before
  syncing.
- Verification requirements were documented in several places but not tied to a
  before-push gate for sync work.
- Historical context and current desired behavior were mixed together, which
  made docs easier to misread during repair.

## Controls Added

| Failure mode | Control |
| --- | --- |
| Upstream history preserved but fork behavior regressed | The global fork-sync skill now requires contract discovery and divergence review. |
| Agents miss repo-local sync policy | `.agents/fork-sync-policy.toml` points to the canonical policy, inventory, gates, and post-mortem. |
| Local contracts are resolved by assumption | The sync ledger requires classification of affected divergences and explicit uncertainty triage. |
| Local build evidence is skipped before push | The sync policy makes the DMG freshness check and local app build gate required before push. |
| Docs are overwritten or misattribute changes | The sync policy requires docs to be checked against the actual diff from the synced upstream baseline. |
| GitHub blockers are misread | The sync policy requires direct inspection of blocking checks, reviews, alerts, and review threads. |

## Current Outcome

Future broad upstream syncs must start from the fork policy, preserve upstream
commit identity, preserve intentional local contracts, record a sync ledger, run
local gates before push, and surface uncertainty for maintainer triage.
