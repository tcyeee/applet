# AGENTS.md

Rules for AI agents working in this repository.

## Project

AI-native desktop runtime (Tauri + React + TS). See [IDEA.md](./IDEA.md) for the
product concept and [TODO.md](./TODO.md) for the phased task list. Apps in this
system are not hand-coded projects — they are **declarative App definitions**
that the runtime installs, persists, and renders. AI agents create/modify apps
by writing App definitions through this schema (eventually via the MCP layer
in TODO step 4), not by generating standalone code.

## App Schema rules

The declarative App Schema lives entirely under `src/app-schema/`:

- `types.ts` — TypeScript types for `AppDefinition` and its parts. This is the
  type-level source of truth.
- `schema.ts` — the Zod schema that mirrors `types.ts` field-for-field and
  enforces cross-references (e.g. a view's `entityId` must exist, entity/field
  ids must be unique). This is the runtime source of truth.
- `validate.ts` — `validateAppDefinition(input)` is the **only** sanctioned way
  to accept an App definition from outside (MCP tool call, file load, etc.).
  It returns `{ success, data | errors }` with human-readable error strings
  designed so an AI agent can self-correct from them.
- `examples/` — hand-written example App definitions (`bookkeeping.json`,
  `habit-tracker.json`) that double as fixtures for `__tests__/validate.test.ts`.

**When changing the schema:**

1. Edit `types.ts` and `schema.ts` together — they must stay in sync field-for-field.
2. Never let UI/runtime code accept or trust an App definition object that
   hasn't gone through `validateAppDefinition`. Don't hand-roll parallel
   validation logic elsewhere.
3. If the change affects the shape example apps rely on, update
   `examples/*.json` in the same change.
4. Run `pnpm test` (vitest) — it re-validates every file in `examples/` — and
   `pnpm lint` before considering the change done.

**Versioning:** `schemaVersion` is currently the literal `"1"`. Additive,
non-breaking changes (new optional fields, new enum values) can land under the
same version. A breaking change (renaming/removing a field, changing meaning)
must introduce a new `schemaVersion` value and keep the old version's
validation path working rather than mutating it in place — the data migration
mechanism for upgrading existing installed apps between versions is future
work (TODO step 2), but the schema itself must not paint that into a corner.

**Naming conventions:** `AppDefinition.id` is kebab-case (e.g.
`personal-bookkeeping`). `entity.id` and `field.id` are camelCase (e.g.
`monthlyLimit`). Both are enforced by regex in `schema.ts`, not just convention.

## Runtime Core

The Rust side of the runtime (App lifecycle, App Registry, per-app SQLite,
data migration, file storage, backup/restore, the automation scheduler) lives
under `src-tauri/src/runtime/`, wired to the frontend via Tauri commands in
`src-tauri/src/commands.rs`:

- `registry.rs` — the App Registry (`registry.sqlite`): install/list/get/
  update/uninstall, status (`stopped`/`running`), and the revision counter.
- `appdb.rs` — turns an App's `dataModel` into real tables in its own SQLite
  file (`apps/<id>/data.sqlite`) and safely migrates them when `dataModel`
  changes: additive changes (new entity, new field) apply automatically;
  anything that would drop or reinterpret data (removed entity/field, changed
  field type) is refused unless the caller passes `force`, in which case the
  affected table is rebuilt and existing values are copied across (cast for
  retyped columns, dropped for removed ones).
- `storage.rs` — per-app private file storage plus a `shared/` directory open
  to all apps. Isolation is structural (each Tauri command only ever touches
  one app's own directory), not permission-gated yet.
- `backup.rs` — packs an app's registry record + database + files into a
  single portable `.zip`, and restores from one.
- `scheduler.rs` — polls running apps' `automations` and emits a
  `runtime://automation` event when a cron trigger matches; it does not
  execute the automation's `action` itself (that's the future action-execution
  engine — TODO step 4).

**The App Schema/Runtime Core trust boundary:** `src-tauri/src/app_schema.rs`
is a Rust mirror of `dataModel` and `automations` used only to generate SQL
and read cron expressions — it is deliberately not a second validator. Every
Tauri command that accepts an `AppDefinition` trusts that the frontend already
ran it through `validateAppDefinition` first. Because a Tauri command is
callable directly from JS (bypassing that TS call is possible, not just
hypothetical), `appdb.rs` still re-checks entity/field ids against a safe
identifier pattern before splicing them into SQL DDL — that check is a SQL
injection guard, not App Schema business-rule validation, and it must not grow
into one.

**When changing the Runtime Core:** run `cargo fmt`, `cargo clippy
--all-targets --all-features`, and `cargo test` from `src-tauri/` before
considering a change done — the test suites in `registry.rs`, `appdb.rs`,
`storage.rs`, `backup.rs`, and `scheduler.rs` cover the install/migrate/
isolate/backup contracts described above.

## MCP Interface Layer

`src-tauri/src/mcp/` exposes the Runtime Core to an AI agent as MCP tools, over stdio, via a
separate binary (`src-tauri/src/bin/mcp_server.rs`) built on the official Rust SDK (`rmcp`) — see
`docs/mcp-server.md` for the full tool reference and how to build/configure it. Runtime decision:
**on-demand stdio, not a daemon bundled into the Tauri GUI app** (see that doc's intro for why);
this settles the "本地 MCP server 常驻，还是按需启动？" question from TODO step 7.

Conventions for adding/changing tools:

- A tool must call straight into `runtime::{registry, appdb, storage, backup}` — the same
  Tauri-independent functions `commands.rs` wraps for the GUI. Do not reimplement runtime logic
  in `mcp/`.
- `mcp/` must never construct or run its own `runtime::scheduler::Scheduler`. That's a polling
  thread tied to a `tauri::AppHandle` for emitting GUI events; a second instance here would
  double-fire automations whenever the desktop app is open at the same time. If a tool needs
  scheduler data, read it from the app's stored `definition.automations` (see `list_automations`),
  don't start a poller.
- `install_app`/`update_app` must validate the incoming `AppDefinition` via
  `mcp::validator::validate_app_definition` (which shells out to the bundled
  `dist-cli/validate-app.mjs`, built from `src/app-schema/validate-cli.ts` by
  `pnpm build:mcp-validator`) before calling `registry::install`/`update_definition`. This is the
  same "TS is the only validator" rule from the App Schema section above — the MCP process is a
  second entry point into trusting an `AppDefinition`, and it must go through the identical gate the
  React frontend does, not a Rust reimplementation of the rules.
- Any tool argument that can lose data (`uninstall_app`'s `purgeData`, `update_app`'s `force`,
  `restore_app`'s `overwrite`) must be paired with a `confirm` argument that also has to be `true`
  before the operation runs — reject with a message describing exactly what would be lost otherwise.
  This is the "危险操作确认机制" TODO step 4 asked for; keep it structural (a required second
  argument), not just wording in the tool's description.
- Tests that exercise `install_app`/`update_app`/anything that calls the validator need
  `dist-cli/validate-app.mjs` built first (`pnpm build:mcp-validator`) — follow the existing
  `bundle_available()` skip-with-message pattern in `mcp/validator.rs` and `mcp/mod.rs`'s tests
  rather than failing outright when it's missing locally; CI always builds it before `cargo test`
  (see `.github/workflows/ci.yml`).
- Run `cargo fmt`, `cargo clippy --all-targets --all-features`, and `cargo test` from `src-tauri/`
  same as any other Runtime Core change.

## Packaging & Distribution

TODO step 6 — see `docs/packaging.md` for the full picture. In short: `.github/workflows/release.yml`
cross-builds installers for macOS/Windows/Linux via `tauri-apps/tauri-action` on a `v*` tag push;
`tauri-plugin-updater` + `tauri-plugin-process` (wired in `src-tauri/src/lib.rs`) give the Runtime
binary itself a self-update path separate from an App's data migration; `src/components/Onboarding.tsx`
is a first-run screen gated by the `get_runtime_info`/`is_onboarding_complete`/`complete_onboarding`
Tauri commands.

**Never commit the updater signing private key.** It's a minisign keypair generated with
`pnpm tauri signer generate`; only the public half (`plugins.updater.pubkey` in `tauri.conf.json`) is
meant to be in the repo. The private half belongs in the `TAURI_SIGNING_PRIVATE_KEY` GitHub Actions
secret (and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` if the key has one), never in a tracked file.
