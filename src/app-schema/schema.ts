import { z } from "zod";
import { SCHEMA_VERSION } from "./types";

const idPattern = /^[a-z][a-zA-Z0-9]*$/;
const appIdPattern = /^[a-z][a-z0-9]*(-[a-z0-9]+)*$/;

const id = z.string().regex(idPattern, "must be camelCase (e.g. \"transactionAmount\")");

const fieldBase = {
  id,
  label: z.string().optional(),
  required: z.boolean().optional(),
  unique: z.boolean().optional(),
};

const simpleFieldSchema = z.object({
  ...fieldBase,
  type: z.enum(["string", "number", "boolean", "date", "datetime"]),
  default: z.union([z.string(), z.number(), z.boolean()]).optional(),
});

const enumFieldSchema = z.object({
  ...fieldBase,
  type: z.literal("enum"),
  options: z.array(z.string()).min(1, "enum field needs at least one option"),
  default: z.string().optional(),
});

const referenceFieldSchema = z.object({
  ...fieldBase,
  type: z.literal("reference"),
  entityId: id,
  cardinality: z.enum(["one", "many"]),
});

export const fieldSchema = z.discriminatedUnion("type", [
  simpleFieldSchema,
  enumFieldSchema,
  referenceFieldSchema,
]);

export const entitySchema = z.object({
  id,
  name: z.string().min(1),
  fields: z.array(fieldSchema).min(1, "entity needs at least one field"),
});

export const dataModelSchema = z.object({
  entities: z.array(entitySchema).min(1, "app needs at least one entity"),
});

const visibleWhenSchema = z.object({
  field: z.string(),
  operator: z.enum(["eq", "neq", "gt", "gte", "lt", "lte"]),
  value: z.union([z.string(), z.number(), z.boolean()]),
});

const viewFieldRefSchema = z.object({
  field: z.string(),
  visibleWhen: visibleWhenSchema.optional(),
});

const chartConfigSchema = z.object({
  groupBy: z.string(),
  aggregate: z.enum(["sum", "count", "avg", "min", "max"]),
  metricField: z.string().optional(),
});

export const viewSchema = z.object({
  id,
  type: z.enum(["list", "form", "detail", "chart"]),
  entityId: id,
  title: z.string().optional(),
  fields: z.array(viewFieldRefSchema).optional(),
  chart: chartConfigSchema.optional(),
});

export const actionSchema = z.object({
  id,
  type: z.enum(["create", "update", "delete", "compute"]),
  entityId: id,
  expression: z.string().optional(),
  targetField: z.string().optional(),
});

const automationActionSchema = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("runAction"), actionId: id }),
  z.object({ kind: z.literal("summarize"), entityId: id, groupBy: z.string() }),
  z.object({ kind: z.literal("notify"), message: z.string() }),
]);

export const automationSchema = z.object({
  id,
  trigger: z.object({ type: z.literal("cron"), expression: z.string().min(1) }),
  action: automationActionSchema,
});

export const appDefinitionSchema = z
  .object({
    schemaVersion: z.literal(SCHEMA_VERSION),
    id: z.string().regex(appIdPattern, "must be kebab-case (e.g. \"personal-bookkeeping\")"),
    name: z.string().min(1),
    description: z.string().optional(),
    dataModel: dataModelSchema,
    views: z.array(viewSchema),
    actions: z.array(actionSchema),
    automations: z.array(automationSchema),
  })
  .superRefine((app, ctx) => {
    const entityIds = new Set<string>();
    for (const [i, entity] of app.dataModel.entities.entries()) {
      if (entityIds.has(entity.id)) {
        ctx.addIssue({
          code: "custom",
          message: `duplicate entity id "${entity.id}"`,
          path: ["dataModel", "entities", i, "id"],
        });
      }
      entityIds.add(entity.id);

      const fieldIds = new Set<string>();
      for (const [j, field] of entity.fields.entries()) {
        if (fieldIds.has(field.id)) {
          ctx.addIssue({
            code: "custom",
            message: `duplicate field id "${field.id}" on entity "${entity.id}"`,
            path: ["dataModel", "entities", i, "fields", j, "id"],
          });
        }
        fieldIds.add(field.id);
      }
    }

    const checkEntityRef = (entityId: string, path: (string | number)[]) => {
      if (!entityIds.has(entityId)) {
        ctx.addIssue({
          code: "custom",
          message: `references unknown entity "${entityId}"`,
          path,
        });
      }
    };

    app.dataModel.entities.forEach((entity, i) => {
      entity.fields.forEach((field, j) => {
        if (field.type === "reference") {
          checkEntityRef(field.entityId, ["dataModel", "entities", i, "fields", j, "entityId"]);
        }
      });
    });

    app.views.forEach((view, i) => checkEntityRef(view.entityId, ["views", i, "entityId"]));
    app.actions.forEach((action, i) => checkEntityRef(action.entityId, ["actions", i, "entityId"]));

    const actionIds = new Set(app.actions.map((a) => a.id));
    app.automations.forEach((automation, i) => {
      if (automation.action.kind === "runAction" && !actionIds.has(automation.action.actionId)) {
        ctx.addIssue({
          code: "custom",
          message: `references unknown action "${automation.action.actionId}"`,
          path: ["automations", i, "action", "actionId"],
        });
      }
      if (automation.action.kind === "summarize") {
        checkEntityRef(automation.action.entityId, ["automations", i, "action", "entityId"]);
      }
    });
  });
