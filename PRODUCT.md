# Product

## Register

product

## Users

Codex App for Linux serves Linux Codex app users who want the official Codex
desktop product experience on a local workstation, especially package-managed
install users who prefer a native package over a one-off generated tree.

It also serves distro/package maintainers and repo maintainers validating
package, updater, and runtime behavior. These users need native package
payloads, updater behavior, local runtime helpers, support routing, and
security evidence to stay auditable. They are often debugging a build, checking
a package, validating an updater path, or deciding whether a visual change
belongs in OpenAI's Codex product, the Linux-port upstream, or this fork.

## Product Purpose

Codex App for Linux preserves the official Codex app experience while this fork
adds the local finishing layer needed for Linux package workflows. The
repository keeps the `codex-app` identity, distro-shaped install layout,
updater policy, hardening posture, port integration defaults, and maintainer
workflow coherent on top of the Linux-port upstream's conversion work.

Success means a user can build, install, launch, update, and troubleshoot a
local Codex desktop app without the fork visually or verbally pretending to own
behavior that still comes from the official OpenAI app bundle and
OpenAI-hosted services. It also means future UI and visual-design work can cite
stable product language before changing layout, copy, screenshots, empty
states, settings surfaces, connected-state presentation, or visual acceptance
criteria.

## Product Surfaces

The official app experience includes the conversation shell, left navigation
rail, pinned and project chat lists, right-side panels, side chat, file preview,
Plugins and Skills directories, Automations, command/search palettes, overflow
menus, and Settings.

Settings are a core product surface, not an afterthought. Future work should
preserve the official app's compact, auditable settings vocabulary across
work-mode choices, appearance controls, approval and sandbox policy, custom
instructions, keyboard shortcuts, MCP servers, hooks, connections, Git,
environments, worktrees, Browser, and Computer Use surfaces. Fork-owned package,
updater, startup, support-routing, and port integration controls should fit
that settings vocabulary when they surface in the app.

## Brand Personality

Codex-native, restrained, precise, trustworthy.

The product should feel like Codex first. Local desktop affordances, package
state, updater details, and port integration controls should fit into the
official app's practical product vocabulary instead of becoming a separate
Linux showcase identity.

## Anti-references

- A generic Linux showcase that centers distro identity, terminal aesthetics,
  or community-port novelty ahead of the Codex product.
- Fake or painted-over screenshots, invented controls, simulated product
  state, fabricated metrics, or UI captures that alter product meaning.
- Mac-only copy in Linux captures or fork-authored UI, including copy that says
  a Linux desktop is a Mac.
- Claims or visuals that imply OpenAI supports Linux as a Codex app platform,
  that this repository redistributes OpenAI software, or that the fork bypasses
  OpenAI-hosted account, rollout, MFA, remote-control, Browser Use, Computer
  Use, or service policy gates.
- Durable docs or PR text that describe this repository with a generic
  Linux-fork label. Describe it as a local hardening and finishing fork layered
  over the Linux-port upstream's work.
- Visual design that treats port integrations as Linux-only capabilities rather
  than configurable build-time modules that adapt official app behavior or local
  runtime helpers to this Linux port.

## Design Principles

1. Codex first. Preserve the official Codex app's product feel, interaction
   density, and practical tone unless this fork owns the surface being changed.
2. Linux context only where useful. Expose package, updater, desktop, and port
   integration details when they help users make correct local decisions.
3. Evidence before polish. Use real generated app output, real source patches,
   real screenshots, and reproducible visual-capture pipelines. Record evidence
   gaps instead of inventing design language.
4. Do not fabricate service or host state. Connected-looking UI must not imply
   enrollment, MFA completion, host liveness, thread visibility, authorization,
   remote environment state, or OpenAI account availability that was not
   actually verified.
5. Keep maintainer surfaces scannable and auditable. Package, updater,
   security, and support-routing information should be quiet, direct, and easy
   to compare during repeated maintenance work.

## Accessibility & Inclusion

Fork-authored UI overlays, docs screenshots, and visual acceptance criteria
should target WCAG 2.2 AA. At minimum, future UI and visual work should preserve
keyboard access, visible focus, sufficient text and state contrast, reduced
motion alternatives, and color-independent status communication.

Visual evidence must be privacy-safe. Committed screenshots and README showcase
assets should use non-sensitive, reproducible staged content and should avoid
private accounts, private paths, private repositories, credentials, hostnames,
tokens, unrelated browser tabs, and visual states that imply unsupported
OpenAI-hosted service behavior.
