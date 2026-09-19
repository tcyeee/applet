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
