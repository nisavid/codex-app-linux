# Support and Issue Routing

Use this guide to decide where to report a bug, request a feature, or attribute
behavior in Codex App for Linux.

## What This Project Provides

This repository does not publish or redistribute the official Codex app. It
provides a recipe that converts the official OpenAI Codex DMG into a local
Linux app, then builds native packages and updater support around that local
build.

Most user-facing app behavior still comes from the official OpenAI app bundle
and OpenAI-hosted services. OpenAI does not support Linux as a Codex app
platform, so Linux-port support depends on the community port, this fork's
packaging layer, and the user's local system.

## Where To Report

Report an issue to [OpenAI's Codex repository](https://github.com/openai/codex)
when it reproduces in the official macOS app, or when the request is
OS-generic and would apply equally to the official app.

> [!IMPORTANT]
> When reporting to OpenAI, reproduce in the official macOS app when possible
> and base the report, screenshots, logs, and terminology on that official app.
> Do not report Linux-port-only behavior as an OpenAI app bug.

Report an issue to
[`ilysenko/codex-desktop-linux`](https://github.com/ilysenko/codex-desktop-linux)
when it reproduces in the Linux-port upstream build, or when the change belongs
to the shared Linux conversion layer that this fork inherits.

> [!IMPORTANT]
> When reporting to the Linux-port upstream, reproduce with a build of
> `ilysenko/codex-desktop-linux` when possible and attach captures or logs from
> that build. Use upstream's names for surfaces that this fork renames; see the
> [rename and compatibility map](../maintainers/fork-divergences.md#current-local-rename-and-compatibility-map)
> for the full mapping.

Report an issue to
[`nisavid/codex-app-linux`](https://github.com/nisavid/codex-app-linux) when it
is specific to this fork's package identity, distro-shaped install layout,
updater policy, hardening, supported default integrations, docs, or local
maintenance workflow. Also report here if you cannot reasonably try the macOS
or Linux-port upstream repro needed for another tracker.

If you are unsure, file the issue here and include enough detail to reroute it:
the app version, build method, distro, desktop session, whether the same
behavior reproduces in the Linux-port upstream build, whether it also
reproduces in the official macOS app, and any reason you could not attempt
those repros.

## Port Integrations

Port integrations are build-time integration modules that adapt official Codex app
surfaces or local runtime helpers to this Linux port. The source directory is
`port-integrations/`.

This fork enables the current supported integration set by default. The default
policy treats these integrations as part of the complete local package, with the
same experimental stability caveats as the rest of the port. Users can disable
an integration when it conflicts with their system or when they want a lighter
build. See [`port-integrations/README.md`](../../port-integrations/README.md) for the
current integration list and config format.

Port integrations do not bypass OpenAI account policy or service-side rollouts. If
a UI surface depends on OpenAI-hosted account state, MFA, connected-client
state, audio availability, or remote-control enrollment, installing this fork
does not change those requirements.
