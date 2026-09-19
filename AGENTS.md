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
