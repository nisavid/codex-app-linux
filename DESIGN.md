---
version: "alpha"
name: "Codex App for Linux"
description: "Codex-native visual guidance for a local hardening and finishing fork layered over the Linux-port upstream."
colors:
  primary: "#1E1E1E"
  on-primary: "#FFFFFF"
  updater-available: "#3A7D44"
  updater-available-border: "#4A9D54"
  updater-devmode: "#6B5300"
  updater-devmode-border: "#A07C00"
  updater-devmode-text: "#FFE9A8"
  updater-sha-chip-text: "#9AA0A6"
typography:
  body-md:
    fontFamily: "-apple-system, BlinkMacSystemFont, Segoe UI, Roboto, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.45
    letterSpacing: "0px"
  label-sm:
    fontFamily: "-apple-system, BlinkMacSystemFont, Segoe UI, Roboto, sans-serif"
    fontSize: "12px"
    fontWeight: 500
    lineHeight: 1.2
    letterSpacing: "0px"
  mono-xs:
    fontFamily: "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace"
    fontSize: "11px"
    fontWeight: 500
    lineHeight: 1.2
    letterSpacing: "0px"
rounded:
  sm: "4px"
  full: "9999px"
spacing:
  xs: "4px"
  sm: "8px"
  md: "16px"
---

# Design System: Codex App for Linux

## Overview

**Creative North Star: "Codex-Native Finishing Layer"**

**Visual Theme & Atmosphere:** Codex App for Linux should read as the official
Codex desktop product running cleanly on Linux: quiet, dense enough for agentic
work, and practical without being austere. The atmosphere is product-led rather
than promotional. Visual attention belongs to the user's task, conversation,
code review, browser automation, package status, or settings choice.

The fork's own visual layer is secondary. Package, updater, Computer Use,
remote-control, and port integration surfaces should feel integrated with the
Codex app shell, not like a separate Linux control panel pasted over it.

Current visual evidence is scoped to source, committed assets, and
privacy-safe inspection:

- The root README uses the committed app icon at `assets/codex.png`, shields,
  and concise user/maintainer routing.
- Privacy-safe local inspection of the live Codex app showed a dark,
  task-focused app shell with a compact left navigation rail, pinned/project
  chat sections, dense conversation rows, subdued dividers, right-side document
  and side-chat panels, command/search palettes, overflow menus, Plugins and
  Skills directories, Automations, and a full Settings mode. No private
  screenshot is committed or attached.
- The observed Settings rail includes General, Appearance, Configuration,
  Personalization, Keyboard shortcuts, MCP servers, Hooks, Connections, Git,
  Environments, Worktrees, Browser, Computer Use, Archived chats, and Usage &
  billing categories. Detailed observations intentionally avoid account-specific
  values, local paths, device names, and private chat content.
- The README visual-capture contract requires future showcase assets to use real
  Codex app surfaces running on Linux with non-sensitive, reproducible staged
  content.
- Fork-authored UI snippets exist for updater status controls, startup
  background handling, settings copy, and remote-control/mobile copy patches.
  The updater overlay colors, button dimensions, typography, transition, and
  SHA chip values are observed in
  `port-integrations/codex-wrapper-updater/patch.js`.

**Key Characteristics:**

- Official Codex product surfaces lead; fork-owned UI stays visually secondary.
- Density is practical and task-focused, not promotional.
- Evidence gaps are explicit so agents do not fabricate service or host state.
- Fork-owned status colors are role-bound and never decorative.

## Colors

**Color Palette & Roles:** Treat these values as observed evidence, not as a
complete official app token dump. The YAML frontmatter exposes only fork-owned
UI tokens that are safe for DESIGN.md-aware tools to reuse; brand icon colors
remain prose-only evidence so agents do not turn them into a default UI palette.

### Primary

- **Linux Startup Charcoal** (`#1E1E1E`) - Opaque startup background applied by
  `scripts/lib/webview-install.sh` to avoid Linux transparency flicker where
  the official OpenAI app bundle expects macOS vibrancy.
- **Foreground White** (`#FFFFFF`) - High-contrast foreground value for
  fork-owned dark startup or overlay contexts.

### Functional States

- **Updater Available Green** (`#3A7D44`, border `#4A9D54`) - Fork-authored
  updater button state for an available Codex App update. Frontmatter token
  keys: `updater-available`, `updater-available-border`.
- **Updater Dev-Mode Amber** (`#6B5300`, border `#A07C00`, text `#FFE9A8`) -
  Fork-authored updater button state for a local build ahead of the official
  OpenAI app bundle version being compared by the updater. Frontmatter token
  keys: `updater-devmode`, `updater-devmode-border`, `updater-devmode-text`.
- **Updater SHA Muted Gray** (`#9AA0A6`, `rgba(120,120,120,0.16)`,
  `rgba(120,120,120,0.28)`) - Fork-authored installed-build chip styling for
  compact metadata. Frontmatter token key: `updater-sha-chip-text`. The rgba
  fill and border values are prose-only evidence because DESIGN.md frontmatter
  colors must stay hex sRGB.

### Evidence-Only Brand Asset Colors

- **Codex Icon Soft Neutral** (`#F7F7F7`, `#F1F1F1`) - Subtle neutral steps
  observed in the icon's rounded square, edge, and highlight treatment.
- **Codex Icon Electric Blue** (`#4165FF`, `#2B14FF`) - Observed saturated
  blue and indigo values in the icon glyph/cloud shape. These are brand-asset
  colors, not default UI accent colors for fork-authored controls.
- **Codex Icon Lavender Highlight** (`#C2B2FF`, `#EAF0FF`) - Observed soft
  highlight and glow values in the icon. Use as icon evidence only unless a
  future visual task extracts a broader official token system.

### Named Rules

**The Icon Evidence Rule.** Icon colors are allowed as evidence for brand asset
recognition, but they are not reusable UI palette tokens unless a future task
extracts official app tokens from generated output.

**The Status Role Rule.** Green, amber, and muted gray belong only to their
documented updater/status roles. If a future screen uses them as decoration, it
is off-system.

### Evidence-Only Official App Theme Controls

The live Appearance settings expose official app theme controls for light,
dark, and system modes; editable accent, background, and foreground roles; UI
font and code font rows; a translucent-sidebar toggle; preview panes; import and
copy-theme actions; and a contrast slider. Treat the visible values in a live
account as user-configurable evidence, not as durable fork-owned tokens.

### Color Rules

Use these rules for all future color work:

- Preserve official Codex app colors when source or generated output exposes
  them.
- Use fork-authored status colors only for their documented status roles.
- Do not turn fork-authored status colors into decoration.
- Use color-independent labels, icons, or text for status changes.
- Record unknown official app token names or generated bundle values as
  evidence gaps rather than inventing a palette.

## Typography

**Body Font:** -apple-system, BlinkMacSystemFont, Segoe UI, Roboto, sans-serif
**Label Font:** -apple-system, BlinkMacSystemFont, Segoe UI, Roboto, sans-serif
**Mono Font:** ui-monospace, SFMono-Regular, Menlo, Consolas, monospace

**Character:** Compact system typography, tuned for a product surface where the
interface should disappear into the task. Type should feel native to Codex and
the host desktop, not branded for its own sake.

**Typography Rules:** Codex App for Linux should follow the official app's
product typography when rendering generated app surfaces. Fork-authored
overlays should use the same system-sans product vocabulary visible in the
source snippets: `-apple-system`, `BlinkMacSystemFont`, `Segoe UI`, `Roboto`,
and `sans-serif`.

Use compact, readable product UI sizing. The updater overlay evidence uses
`12px` medium text for the update button and `11px` medium monospace text for
the SHA chip. Treat that as a narrow overlay precedent, not as a general page
scale.

Use monospace only for commit hashes, paths, versions, package names, commands,
or other machine-readable metadata. Avoid display fonts, ornamental type, and
oversized marketing hierarchy in app or maintainer surfaces.

The observed official Appearance settings expose separate UI and code font
controls. Future fork-owned UI should not copy live user font values as product
tokens unless a task verifies they are official defaults or extracts them from
generated app output.

### Hierarchy

- **Body** (400, `14px`, line-height 1.45): Default prose and settings text
  for fork-owned UI.
- **Label** (500, `12px`, line-height 1.2): Compact buttons, small metadata,
  and status labels such as the updater action.
- **Mono** (500, `11px`, line-height 1.2): Commit hashes, versions, package
  names, paths, and other machine-readable metadata.

### Named Rules

**The Native Type Rule.** Fork-owned UI uses the system-sans stack unless
generated output exposes official Codex typography tokens. Do not introduce a
display face, decorative serif, or marketing hierarchy.

## Layout

**Layout Principles:** Use product UI density. The user is usually in an active
workflow: launching, chatting, reviewing changes, staging browser automation,
checking package state, configuring port integrations, or validating a host
boundary.

Keep fork-owned UI visually subordinate to the main Codex work surface.
Controls that belong to package updates, wrapper state, or port integrations
should live in the smallest appropriate setting, header, or status location and
should not take over the screen.

The live app uses a stable product shell: a left rail for primary navigation and
chat/project lists, a central work surface for conversations or directory
content, and an optional right utility panel with tabs for file preview and side
chat. Settings switch into a full-screen mode with a persistent left settings
rail and a centered, fixed-width content column.

Directory surfaces such as Plugins, Skills, and Automations keep content
centered and scannable. Plugins and Skills use search/filter controls and
compact two-column rows; Automations uses a centered empty state with suggested
actions.

README and documentation visuals should stay compact and scannable. Future
showcase images should be a single composed asset near the top of `README.md`,
stored under `docs/assets/readme/`, and based on a reproducible capture
pipeline with staged non-sensitive content.

Responsive behavior should preserve readable text, visible focus, and standard
hit targets. Do not use oversized hero typography, marketing card grids,
decorative gradients, or generic Linux desktop ornamentation for product or
maintainer surfaces.

## Elevation & Depth

Codex App for Linux should prefer flat, tonal layering over decorative depth.
Fork-authored overlays should use borders, muted fills, and compact placement
before shadows. If elevation is needed for a menu, tooltip, or transient
control, keep it functional and subordinate to the official Codex work surface.

The observed command palette and overflow menu use functional elevation: a
floating dark surface, section labels, selected-row fill, separators, shortcut
metadata, disabled rows, and submenu arrows. Reuse this kind of layering for
transient controls instead of decorative cards or large shadows.

Loading, installation, and rebuild states should prioritize clear progress,
cancellation/error handling, and recovery over visual flourish. Do not add
orchestrated page-load animations to product workflows.

## Shapes

The fork-authored shape language should stay restrained and product-native. The
updater overlay evidence uses a low `4px` radius for compact controls and
pill-like geometry only where metadata or status chips need quick recognition.

Do not introduce decorative cards for maintainer docs or product UI. Use
containers, dividers, and spacing only when they help scan repeated controls or
separate state groups.

Observed official app rows are subtly rounded and grouped by function: sidebar
items, settings cards, plugin rows, keyboard shortcut rows, and worktree rows.
Fork-authored UI should keep rounded geometry restrained and reserve pill
shapes for compact metadata, toggles, or shortcut chips.

## Components

**Component Stylings:** Follow the official Codex component vocabulary where
available. Fork-authored components should be compact, direct, keyboard
reachable, visibly focusable, and color-independent.

### Buttons

Fork-authored micro-controls should be labeled with the action they perform.
The updater button evidence is 22px tall, horizontally padded, low radius
(`4px`), and status-colored only when an update or dev-mode state is present.

Destructive actions should follow the observed app pattern: explicit red action
text or red-toned buttons, placed at row ends or near the relevant setting. Do
not hide destructive package, updater, worktree, or reset behavior behind
ambiguous icons.

### App Sidebar and Chat Rows

The live sidebar combines icon-leading primary navigation, pinned chat rows,
project sections, timestamps, status/branch metadata, and selected-row fill.
Keep future fork-owned sidebar additions compact and text-first. Use icons to
support scanning, but do not add decorative badges or Linux-themed ornament.

### Right Panels and Tabs

The observed right side panel uses a tab strip for document preview and side
chat, with a plus affordance for adding panels. File preview shows a dark
reading surface, breadcrumbs, metadata, and rendered markdown. Side chat can be
present as a quiet blank panel when no side-chat content exists. Fork-authored
panel content should preserve this utility-panel role instead of competing with
the main conversation.

### Command Palette and Menus

Command/search palettes should stay centered, compact, and keyboard-first:
input at the top, section labels, icon-leading rows, selected-row fill, and
shortcut chips on the right. Overflow menus should use the same dark product
surface, clear labels, separators, disabled states, submenu arrows, and shortcut
metadata.

### Extension Directories

Plugins and Skills use a segmented tab control, search field, filter chips,
featured or recommended areas, two-column extension rows, installed checkmarks,
and add buttons. Extension rows should show an icon, name, short description,
and one clear state/action. Do not turn plugin or skill lists into promotional
cards.

### Automations

The observed Automations surface uses a centered title/subtitle, a simple empty
state icon, concise empty-state copy, and compact suggestion buttons. Future
automation UI should expose schedule or trigger state directly and avoid
implying that a job exists or ran unless that state is verified.

### Status Chips

Use muted, compact chips for metadata such as installed build hashes. The SHA
chip precedent uses a small monospace label, subdued gray text, translucent gray
fill, and a subtle border.

### Settings Rows

Settings added by this fork should read like ordinary product settings: label,
short description, standard toggle/control, and inline error text when needed.
Settings copy should state Linux desktop, package, updater, or port integration
behavior directly.

The live Settings mode uses a left category rail, a centered content column,
section headings, grouped rows, toggles, dropdowns, segmented controls, sliders,
search fields, text areas, keyboard shortcut chips, add buttons, manage buttons,
refresh affordances, warning banners, and explicit destructive controls. Fork
settings should reuse that vocabulary and group capability, data, policy, and
recovery controls separately.

Permission and capability surfaces must stay risk-aware. Configuration, Browser,
Computer Use, Connections, and Git settings show approval choices, sandbox or
access controls, device/app rows, empty lists, and recovery/destructive actions.
Fork-owned controls for updater, packages, or port integrations should make the
local permission boundary as visible as the action itself.

### Inputs and Forms

Preserve official app form controls where generated surfaces provide them.
Fork-authored inputs should keep labels above or next to controls, expose error
text inline, and retain keyboard and focus states.

Large editable instruction areas and commit/PR guidance fields use monospaced
text, scrollable boxes, and explicit Save actions. Do not autosave durable
policy or maintainer guidance without a visible saved/unsaved state.

### Empty, Loading, and Error States

Empty states should explain the next local action without implying service
availability. Loading states should describe the local operation when possible,
such as checking updater state or waiting for an app-server/host path. Error
states must distinguish local fork behavior from OpenAI-hosted service policy.

### Motion and Interaction

Motion should communicate state, not decoration. Fork-authored controls should
use short transitions comparable to the updater overlay's `120ms`
background-color transition.

Respect reduced-motion preferences. Interactions that imply remote-control,
mobile setup, Computer Use, or connected-host state must be backed by real
account, host, app-server, device key, and thread/session evidence. A
connected-looking state is not enough by itself.

## Evidence Gaps

- No generated `codex-app/` tree is present in this checkout, so official
  generated bundle tokens, exact app CSS variables, and generated settings
  layouts are not committed here as durable evidence.
- No committed README showcase asset exists under `docs/assets/readme/` yet.
  Future assets should follow `docs/maintainers/readme-visual-capture.md`.
- Authenticated OpenAI-hosted surfaces, remote-control enrollment, Codex mobile
  state, Browser Use policy gates, and Computer Use account gates are external
  to this fork unless a task records specific live evidence.
- Local Computer Use inspection can confirm current desktop rendering and
  accessibility/screenshot readiness, but private maintainer screenshots should
  remain inspection evidence unless a reproducible non-sensitive capture
  pipeline is created.
- This task observed Settings category structure, component vocabulary, and
  side-panel behavior. It did not record private account values, device names,
  local paths, chat titles, or Usage & billing details as durable evidence.

## Do's and Don'ts

### Do

- Do preserve the official Codex app's product feel, interaction density, and
  practical tone unless this fork owns the surface being changed.
- Do expose package, updater, desktop, and port integration details only when
  they help users make correct local decisions.
- Do use real generated app output, real source patches, real screenshots, and
  reproducible visual-capture pipelines.
- Do model future settings, side panels, command palettes, and extension
  directories on the official Codex app patterns observed in live inspection.
- Do target WCAG 2.2 AA for fork-authored UI overlays, docs screenshots, and
  visual acceptance criteria.
- Do use color-independent labels, icons, or text for status changes.

### Don't

- Don't use a generic Linux-fork label for this repository in durable docs or
  PR text.
- Don't create a generic Linux showcase that centers distro identity, terminal
  aesthetics, or community-port novelty ahead of the Codex product.
- Don't describe port integrations as Linux-only capabilities.
- Don't invent screenshots, metrics, connected clients, host liveness,
  enrollment state, MFA state, remote environments, simulated product state, or
  OpenAI service availability.
- Don't imply OpenAI supports Linux as a Codex app platform, that this
  repository redistributes OpenAI software, or that this fork bypasses
  OpenAI-hosted account, rollout, MFA, remote-control, Browser Use, Computer
  Use, or service policy gates.
- Don't fake or paint over screenshots, invented controls, wrong product copy,
  or UI captures that alter product meaning. Fix the source patch or choose a
  different capture.
- Don't use Mac-only copy for Linux desktop behavior.
- Don't use app icon blue/lavender values as broad decorative gradients for
  fork-authored UI.
- Don't use decorative Linux theming, terminal cosplay, oversized hero
  sections, generic card grids, or promotional copy in product surfaces.
- Don't commit screenshots that expose private accounts, paths, repositories,
  conversations, hostnames, tokens, credentials, unrelated browser tabs, or
  service states that were not verified.
