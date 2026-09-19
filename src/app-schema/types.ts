/**
 * TypeScript types for the declarative App Schema.
 *
 * These types mirror the Zod schema in `schema.ts` field-for-field —
 * `schema.ts` is the runtime source of truth, this file exists for
 * compile-time ergonomics when authoring/consuming App definitions in TS.
 */

export const SCHEMA_VERSION = "1" as const;

export type FieldType =
  | "string"
  | "number"
  | "boolean"
  | "date"
  | "datetime"
  | "enum"
  | "reference";

interface FieldBase {
  id: string;
  label?: string;
  required?: boolean;
  unique?: boolean;
}

export type Field =
  | (FieldBase & { type: "string" | "number" | "boolean" | "date" | "datetime"; default?: string | number | boolean })
  | (FieldBase & { type: "enum"; options: string[]; default?: string })
  | (FieldBase & { type: "reference"; entityId: string; cardinality: "one" | "many" });

export interface Entity {
  id: string;
  name: string;
  fields: Field[];
}

export interface DataModel {
  entities: Entity[];
}

export type ViewType = "list" | "form" | "detail" | "chart";

export interface VisibleWhen {
  field: string;
  operator: "eq" | "neq" | "gt" | "gte" | "lt" | "lte";
  value: string | number | boolean;
}

export interface ViewFieldRef {
  field: string;
  visibleWhen?: VisibleWhen;
}

export interface ChartConfig {
  groupBy: string;
  aggregate: "sum" | "count" | "avg" | "min" | "max";
  metricField?: string;
}

export interface View {
  id: string;
  type: ViewType;
  entityId: string;
  title?: string;
  fields?: ViewFieldRef[];
  chart?: ChartConfig;
}

export type ActionType = "create" | "update" | "delete" | "compute";

export interface Action {
  id: string;
  type: ActionType;
  entityId: string;
  /** Restricted arithmetic expression over the entity's own fields, e.g. "amount * quantity". Only used by "compute" actions. */
  expression?: string;
  targetField?: string;
}

export interface Automation {
  id: string;
  trigger: { type: "cron"; expression: string };
  action:
    | { kind: "runAction"; actionId: string }
    | { kind: "summarize"; entityId: string; groupBy: string }
    | { kind: "notify"; message: string };
}

export interface AppDefinition {
  schemaVersion: typeof SCHEMA_VERSION;
  id: string;
  name: string;
  description?: string;
  dataModel: DataModel;
  views: View[];
  actions: Action[];
  automations: Automation[];
}
