# AWS Login

Libra reference app for AWS SSO session management. See `architecture/plans/PLAN-AWS-LOGIN.md`. Packaging follows [[20-packaging|§20]] (`architecture/sections/20-packaging.md`).

## Install (end users)

You need a build artifact for your platform — from a release or from `libra package` on that OS (below).

### macOS (DMG)

1. Open `AWS Login_<version>_aarch64.dmg`.
2. Drag **AWS Login.app** into **Applications**.
3. Eject the disk image.
4. Launch from Applications (or Spotlight).

**Signed builds** (Developer ID + notarization configured at release time) open normally on any Mac.

**Unsigned builds** (default local `libra package` output) may be blocked by Gatekeeper on another Mac.

If macOS says the app is **“damaged and can’t be opened”**, that is usually Gatekeeper + an unsigned transfer — not file corruption. Try, on the **receiving** Mac:

```sh
xattr -cr "/Applications/AWS Login.app"
```

Then launch again, or **Right-click → Open → Open** once.

If it still fails, confirm the build is **Apple Silicon** (`aarch64` in the DMG name) and the target Mac is too — Intel Macs need an x86_64 build.

For ad-hoc sharing without the “damaged” false alarm, rebuild with a current `libra package` (it ad-hoc signs the bundle after packager). Production releases should use `[package.sign.macos]` (see below).

### Windows (NSIS)

1. Run the installer from `dist/` (e.g. `AWS Login_<version>_x64-setup.exe`).
2. Follow the installer; the app installs under `%LOCALAPPDATA%` by default.
3. Launch **AWS Login** from the Start menu.

Optional Authenticode signing: set `[package.sign.windows].thumbprint` in `libra.toml` before packaging on Windows.

### Linux (deb or AppImage)

**deb:** install with your package manager, then launch `aws-login` from the app menu:

```sh
sudo dpkg -i dist/aws-login_<version>_amd64.deb
```

**AppImage:** make executable and run (no install step):

```sh
chmod +x dist/AWS\ Login_<version>_amd64.AppImage
./dist/AWS\ Login_<version>_amd64.AppImage
```

### Requirements (all platforms)

- Network access for AWS SSO authentication
- `aws` CLI discoverable on `PATH` if you use credential export (the packaged app augments `PATH` with common install locations — Homebrew, `/usr/local/bin`, Chocolatey, Scoop, etc.)

No Rust toolchain required on the install machine — only the packaged artifact.

## Development

From the repo root (requires Rust + the Libra CLI):

```sh
cargo install --path cli/libra --force   # one-time
libra dev applications/aws-login
```

When `libra.toml` is present, `libra dev` loads `[package].icon` for the window title-bar icon (same PNG used in packaged builds).

## Release build

```sh
libra build -p aws-login
```

Produces `target/release/aws-login` with release view codegen and LTO (§18.2).

## Packaging

Prerequisite (one-time, on the **build** machine):

```sh
cargo install cargo-packager --locked
```

Run on the **host OS** you are targeting (`--target` must match the machine you are on):

```sh
libra package -p aws-login                    # host defaults: .app+.dmg / NSIS / deb+AppImage
libra package -p aws-login --format app       # single format override
libra package -p aws-login --target windows   # on Windows only
libra package -p aws-login --target linux     # on Linux only
```

### macOS

Output: `applications/aws-login/dist/AWS Login.app` and `dist/AWS Login_<version>_aarch64.dmg`.

### Windows

Output: NSIS installer under `applications/aws-login/dist/` (e.g. `AWS Login_<version>_x64-setup.exe`).

Optional signing in `libra.toml`:

```toml
[package.sign.windows]
thumbprint = "YOUR_CERT_SHA1_THUMBPRINT"
```

### Linux

Output: `dist/aws-login_<version>_amd64.deb` and `dist/AWS Login_<version>_amd64.AppImage`.

Identity fields live in `libra.toml` (`bundle_id`, `app_user_model_id`). `libra package` also writes `packaging/libra-identity.toml` and bundles it for native integrations (Windows toast AppUserModelID, launch-at-login, etc.).

### `libra.toml` (sample)

The committed manifest beside this crate is the v1 reference — copy and edit for new apps:

```toml
[app]
name = "AWS Login"                    # display name in Finder / DMG
bundle_id = "dev.libra.aws-login"     # macOS CFBundleIdentifier
app_user_model_id = "Libra.AwsLogin" # Windows toast AppUserModelID

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

### CI

`libra publish --ci` writes `.github/workflows/release.yml` from `libra.toml`. The workflow runs `libra package`, then `libra publish --from-dist`. Secrets, when signing is configured: `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`, `APPLE_ID`, `APPLE_APP_PASSWORD`, `APPLE_TEAM_ID`. Add `LIBRA_UPDATE_KEY` only when `[package.update]` is set.

`libra package` on this Mac stays the backup. It does not need `gh`. With the keychain notary profile it still notarizes; without a sign table it ad-hoc signs.

Unsigned builds remain valid for local testing; signing is never required for development.

## Tests

```sh
cargo test -p aws-login
LIBRA_VALIDATE=1 cargo test -p aws-login --test snapshots  # re-record PNGs
```
