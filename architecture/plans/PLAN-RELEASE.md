---
title: "PLAN-RELEASE"
aliases:
  - "PLAN-RELEASE"
tags:
  - plans
  - architecture
type: plan
---

# Icons, publish, and a signed update feed (§20)

**Status: shipped.** Implements the release commands in [[18.1-cli|§18.1]] and the
update contract in [[20-packaging|§20]]. Amends the v0 auto-updater line in
[[24-non-goals|§24]].

`libra package` stays the local backup. It never calls `gh`. With
`[package.sign.macos]` and a keychain notary profile it still notarizes on
this Mac. Without a sign table it ad-hoc signs.

## Contract

| Table | When absent |
|---|---|
| `[package.sign.macos]` | Ad-hoc bundle |
| `[package.publish]` | Nothing to upload |
| `[package.update]` | The shell never checks |

`libra new` writes the publish and update tables commented out.

## Commands

| Command | What it does |
|---|---|
| `libra icon <png> [-o dir]` | Square PNG, at least 512 px, to png / icns / ico / hicolor |
| `libra package` | Local bundle. Passes generated icns and ico through. Bundles the update pubkey in `packaging/libra-identity.toml` when the table is set |
| `libra publish` | Package (unless `--from-dist`), sign the archive with `LIBRA_UPDATE_KEY`, upload the bundle and `latest.json` with `gh` |
| `libra publish --no-upload` | Write the feed next to `dist/` and stop |
| `libra publish --ci` | Write `.github/workflows/release.yml` and stop. The workflow runs `libra package`, then `libra publish --from-dist` |
| `libra publish --keygen` | Print a pubkey for `libra.toml` and a secret for `LIBRA_UPDATE_KEY` |

The author still enrolls with Apple, installs the Developer ID certificate, and
runs `xcrun notarytool store-credentials <notary_profile>` by hand. CI secrets:
`APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`,
`APPLE_APP_PASSWORD`, `APPLE_TEAM_ID`, and `LIBRA_UPDATE_KEY` when updates are on.

## Shell

On launch, a packaged macOS app whose identity resource has an endpoint and a
pubkey fetches `latest.json`, and on accept downloads the archive, verifies
the minisign signature, replaces the `.app`, and relaunches. Decline leaves
the app running. The first target the feed is built for is `darwin-aarch64`.
The JSON has a slot per host. `LIBRA_NO_UPDATE=1` skips the check. Tests use
fixtures. They never call `notarytool`, `gh`, or a live update server.

---

← [[Plans Index]] · [[20-packaging]] · [[18.1-cli]]
