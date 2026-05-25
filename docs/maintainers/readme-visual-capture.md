# README Visual Capture

Use this guide when producing showcase visuals for the project README.

The README should stay clean, utilitarian, and accurate for users and distro
maintainers. Visuals should show real Codex app surfaces running on Linux, with
non-sensitive and reproducible staged content.

## Output Contract

Future README showcase visuals should be a single compact composite image near
the top of `README.md`, after the introductory note. Prefer one composed asset
over several inline screenshots so the README remains scannable on GitHub
desktop and mobile.

Store committed README showcase assets under `docs/assets/readme/` and reference
them with non-empty alt text. The existing app icon remains `assets/codex.png`
because package and app builders use that path as an input.

Run the README visual reference check before committing README visual changes:

```bash
node scripts/ci/validate-readme-visuals.js README.md
```

## Capture Standard

Committed showcase assets should come from a checked-in, reproducible capture
pipeline. Manual screenshots are useful for exploration, but final README
assets should be produced from repeatable maintainer commands that set up a
non-sensitive demo workspace, drive the app to known UI states, capture stills,
compose the final image, and document any polish or redaction.

The pipeline does not need to run fully in CI while it depends on a local app,
local desktop session, account-gated app state, or maintainer credentials. It
does need to make inputs, commands, and required local prerequisites explicit
enough for another maintainer to reproduce the result.

Use real UI states. Do not invent controls, paste in fake product state, simulate
notifications, or edit screenshots in ways that change product meaning.

Allowed image edits:

- crop and resize captures for composition;
- adjust color or contrast for legibility;
- redact or mask sensitive values;
- balance the composite so the README image is readable at GitHub widths.

If a label or UI state is wrong for Linux, fix the source patch or choose a
different capture. Do not paint over incorrect product copy.

## Shot List

Treat this list as priority guidance, not a fixed collage template. Promote or
demote candidates when a new app version introduces stronger visual states.

Must-capture candidates:

- Main Codex workbench on a Linux desktop, staged with a populated sidebar,
  colorful pull request indicators, in-chat change summaries, and non-sensitive
  conversation content.
- Browser Use with annotations, staged against public or disposable web content.

Strong candidates:

- Diff or change-review view with a readable color scheme.
- Remote Control settings after validating the generated Linux app no longer
  presents Mac-only copy in the captured surface.
- Settings side-panel views that show Linux-relevant app integration without
  exposing private account details.

Optional or future candidates:

- Updater notification or status sequence while building a new package from a
  freshly fetched official OpenAI Codex DMG, once the local updater path is
  healthy enough to capture reproducibly.
- Plugin or skill browser surfaces when they add visual value without distracting
  from the main product story.

Do not prioritize Computer Use backend output for still images unless a specific
UI state is visually compelling. Computer Use is important, but readiness and
desktop-control reports are usually better documented as text or video.

## Demo Workspace Requirements

Use staged public or disposable content for captures:

- public repository paths or synthetic local paths;
- synthetic issue and pull request names;
- disposable conversation text;
- public web pages or local demo pages for Browser Use;
- no private account identifiers, profile images, credentials, tokens, API keys,
  hostnames, user names, private file paths, private conversations, or private
  browser/session state.

The staged workspace should be reusable. Prefer scripts and checked-in fixtures
over one-off personal sessions.

## Security And Privacy Review

Before committing showcase assets, inspect every source capture and final
composite for:

- credentials, tokens, API keys, cookies, bearer strings, and session IDs;
- account names, email addresses, user names, avatars, organization names, and
  private repository names;
- private host paths, shell history, prompt content, logs, and environment
  values;
- private conversations, screenshots of unrelated apps, browser tabs, and
  window titles;
- UI states that imply this fork bypasses OpenAI-hosted service gates, account
  policy, remote-control enrollment, MFA, Browser Use policy, or Computer Use
  host prerequisites.

Metadata inspection is a reviewer responsibility until an asset-producing PR
adds automated checks for the chosen file formats. Strip EXIF, XMP, IPTC, PNG
text chunks, and similar metadata when producing final assets.

## Validation Scope

`scripts/ci/validate-readme-visuals.js` checks README image references. It
allows the existing app icon and shields.io badges, allows zero showcase images,
and validates future README showcase references for approved path and alt-text
hygiene.

The validator does not inspect pixels, metadata, or composition quality. Those
checks belong to the capture pipeline and maintainer review for the
asset-producing PR.
