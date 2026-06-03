# Fork Sync Ledger

This directory holds durable summaries for broad syncs from the Linux-port
upstream into this fork. Keep the PR body concise, but copy the final sync
ledger here before closeout so future syncs can review prior imported behavior,
special handling, and follow-up decisions without searching old PR text.

Use one file per broad sync:

```text
YYYY-MM-DD-pr-NN-upstream-SHORTSHA.md
```

Each entry should include:

- sync scope: PR, merge commit, base commit, previous baseline, and synced
  Linux-port upstream commit;
- upstream commit catalog grouped by behavior area;
- local reconciliation notes for renamed paths and fork contracts;
- user-facing or maintainer-facing highlights that may need special handling;
- follow-up decision for each special-handling item, including links to existing
  issues or a note that no new issue is warranted;
- verification evidence from local gates and final PR checks.

Do not record secrets, local-only credentials, or full generated artifacts in
the ledger. Link to PRs, issues, docs, and commands instead.
