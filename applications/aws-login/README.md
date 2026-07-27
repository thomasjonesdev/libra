# AWS Login

Libra reference app for AWS SSO session management. See `architecture/plans/PLAN-AWS-LOGIN.md`. Packaging follows [[20-packaging|§20]] (`architecture/sections/20-packaging.md`).

## Install (end users)

You need a **macOS** build artifact — either a `.dmg` from a release or one you package locally (below).

### From a release DMG

1. Open `AWS Login_<version>_aarch64.dmg`.
2. Drag **AWS Login.app** into **Applications**.
3. Eject the disk image.
4. Launch from Applications (or Spotlight).

**Signed builds** (Developer ID + notarization configured at release time) open normally on any Mac.

**Unsigned builds** (default local `libra package` output) may be blocked by Gatekeeper on first launch:

- **Right-click** the app → **Open** → confirm **Open** once, or
- **System Settings → Privacy & Security** → **Open Anyway** after the first blocked attempt.

This is expected for ad-hoc sharing; production releases should use `[package.sign.macos]` (see below).

### Requirements

- macOS 11 or later (Apple Silicon build shown; Intel requires packaging on that arch)
- Network access for AWS SSO authentication
- `aws` CLI on `PATH` if you use credential export (app probes this at runtime)

No Rust toolchain required on the install machine — only the packaged `.app` / DMG.

## Development

From the repo root (requires Rust + the Libra CLI):

```sh
cargo install --path cli/libra --force   # one-time
libra dev -p aws-login
```

When `libra.toml` is present, `libra dev` loads `[package].icon` for the window title-bar icon (same PNG used in packaged builds).

## Release build

```sh
libra build -p aws-login
```

Produces `target/release/aws-login` with release view codegen and LTO (§18.2).

## Packaging (macOS)

Prerequisite (one-time, on the **build** machine):

```sh
cargo install cargo-packager --locked
```

Unsigned bundle (local sharing):

```sh
libra package -p aws-login
```

Output: `applications/aws-login/dist/AWS Login.app` and `dist/AWS Login_<version>_aarch64.dmg`.

Identity fields live in `libra.toml` (`bundle_id`, `app_user_model_id`) for downstream native integrations (notifications, launch-at-login).

### `libra.toml` (sample)

The committed manifest beside this crate is the v1 reference — copy and edit for new apps:

```toml
[app]
name = "AWS Login"                    # display name in Finder / DMG
bundle_id = "dev.libra.aws-login"     # macOS CFBundleIdentifier
app_user_model_id = "Libra.AwsLogin" # Windows toast ID (P6)

[package]
icon = "assets/icon.png"              # square PNG; one source for all platform sizes

# Optional — Gatekeeper-friendly releases (P4)
# [package.sign.macos]
# identity = "Developer ID Application: Your Name (TEAMID)"
# notary_profile = "libra-notary"
# provider_short_name = "YourTeamShortName"
```

Window title and size stay on `libra::app!` in Rust for v1; the manifest carries identity and packaging only.

## Signing and notarization (optional)

Gatekeeper-friendly distribution requires a **Developer ID Application** certificate and Apple notarization. Configure signing in `libra.toml`:

```toml
[package.sign.macos]
identity = "Developer ID Application: Your Name (TEAMID)"
notary_profile = "libra-notary"
provider_short_name = "YourTeamShortName"  # optional; Apple Developer team short name
```

### One-time setup

1. Install your Developer ID certificate in the login keychain (Xcode → Settings → Accounts, or Apple Developer portal).
2. Store notary credentials in the keychain:

   ```sh
   xcrun notarytool store-credentials libra-notary \
     --apple-id "you@example.com" \
     --team-id TEAMID \
     --password "@keychain:AC_PASSWORD"
   ```

   Use an app-specific password or keychain item for `AC_PASSWORD`. The profile name (`libra-notary`) must match `notary_profile` in `libra.toml`.

3. Package with signing enabled:

   ```sh
   libra package -p aws-login
   ```

   `libra package` writes `signing-identity` (and optional `provider-short-name`) into the generated `Packager.toml`, sets `APPLE_KEYCHAIN_PROFILE` from `notary_profile`, and delegates codesign + notarization to cargo-packager.

### CI recipe (GitHub Actions sketch)

- Import the signing certificate via `apple-actions/import-codesign-certs`.
- Store the notary profile on the runner keychain before `libra package` (same `notarytool store-credentials` step, secrets for Apple ID + app password + team id).
- Run `libra package -p aws-login` on a `macos-latest` runner.
- Upload `applications/aws-login/dist/*.dmg` as a release artifact.

Unsigned builds remain valid for local testing; signing is never required for development.

## Tests

```sh
cargo test -p aws-login
LIBRA_VALIDATE=1 cargo test -p aws-login --test snapshots  # re-record PNGs
```
