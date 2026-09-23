# Packaging & Distribution

Covers TODO step 6: cross-platform packaging, the Runtime's own self-update mechanism, and
first-run onboarding. All three are about the **Applet Runtime binary itself** — a separate concern
from an installed App's declarative definition or its data migration (`src-tauri/src/runtime/appdb.rs`,
see AGENTS.md's Runtime Core section).

## Cross-platform packaging

`src-tauri/tauri.conf.json`'s `bundle` section (`targets: "all"`) produces, per OS, whatever Tauri
bundles natively: `.app`/`.dmg` on macOS, `.msi`/NSIS `.exe` on Windows, `.deb`/`.rpm`/AppImage on
Linux. Building all three requires each native toolchain, which a single CI runner doesn't have —
`.github/workflows/release.yml` builds them in a matrix (`macos-latest` ×2 targets for Apple
Silicon/Intel, `ubuntu-22.04`, `windows-latest`) via `tauri-apps/tauri-action`, triggered by pushing
a `v*` tag, and attaches the resulting installers to a GitHub Release, which it publishes automatically.

To cut a release: bump `version` in `src-tauri/tauri.conf.json`, tag (`git tag vX.Y.Z && git push
--tags`), and let the workflow run — the release goes live (and the updater picks it up) as soon as
all matrix jobs finish, with no manual publish step.

## Self-update mechanism

Runtime version upgrades (distinct from an App's data migration) go through `tauri-plugin-updater` +
`tauri-plugin-process` (added in `src-tauri/Cargo.toml` / `package.json`, wired in `src-tauri/src/lib.rs`,
granted `updater:default`/`process:default` in `src-tauri/capabilities/default.json`):

- `src/components/UpdateChecker.tsx` calls `check()` → `update.downloadAndInstall()` → `relaunch()`;
  it's mounted in the App Picker toolbar (`src/App.tsx`).
- `tauri.conf.json`'s `bundle.createUpdaterArtifacts: true` makes the build produce signed update
  packages + a manifest alongside the normal installers.
- `plugins.updater.endpoints` points at `https://github.com/tcyeee/applet/releases/latest/download/latest.json`
  — the file `tauri-action` publishes automatically onto the GitHub Release it creates, so no
  separate update server is needed.
- `plugins.updater.pubkey` is the public half of a minisign keypair generated locally with
  `pnpm tauri signer generate` (the private half was written to `.context/updater-keys/` — gitignored,
  **not** committed). The matching private key must be added to the repo's GitHub Actions secrets as
  `TAURI_SIGNING_PRIVATE_KEY` (and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` if one was set) before
  `release.yml` can produce artifacts the updater will accept — `tauri-action` signs update packages
  with it, and `tauri-plugin-updater` on the client verifies the signature against `pubkey`. If the
  key is ever lost or rotated, `pubkey` in `tauri.conf.json` must be updated in the same release that
  switches signing keys, or existing installs won't trust the new signature.

## First-run onboarding

`src/components/Onboarding.tsx` gates the app on first launch (`App.tsx` checks
`client.isOnboardingComplete()` before rendering the App Picker). It doesn't need to initialize
anything itself — `RuntimeState::init` (`src-tauri/src/lib.rs`) already creates `registry.sqlite` and
the data directory synchronously during Tauri's `setup` hook, before any command can run — so the
screen's job is purely informational plus recording that it's been seen:

- Shows the resolved data directory (`get_runtime_info` Tauri command) so the user can point an MCP
  client at the same one via `APPLET_DATA_DIR` (see `docs/mcp-server.md`).
- Renders a ready-to-copy MCP client config snippet using that same data directory.
- `complete_onboarding` writes a marker file (`<data dir>/onboarding-complete`) so the screen is
  skipped on subsequent launches. This lives next to `registry.sqlite`, not inside it or any App's
  data — it's Runtime-level UI state, not App Registry or App data, so it doesn't belong in either of
  those SQLite databases.
