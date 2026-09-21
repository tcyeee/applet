import { z, type ZodTypeAny } from "zod";
import type { Entity, Field, ViewFieldRef } from "@/app-schema/types";

function baseTypeSchema(field: Field): ZodTypeAny {
  switch (field.type) {
    case "string":
    case "date":
    case "datetime":
      return z.string();
    case "number":
      return z.coerce.number();
    case "boolean":
      return z.coerce.boolean();
    case "enum":
      return z.enum(field.options as [string, ...string[]]);
    case "reference":
      return z.string();
  }
}

function applyRequired(schema: ZodTypeAny, field: Field): ZodTypeAny {
  if (field.required) {
    if (field.type === "string" || field.type === "date" || field.type === "datetime") {
      return (schema as z.ZodString).min(1, `${field.label ?? field.id} 不能为空`);
    }
    return schema;
  }
  return schema.optional().or(z.literal(""));
}

/**
 * Builds a Zod object schema for the fields a view exposes, driven by the
 * underlying entity's field definitions (type + required). Used by FormView
 * so record edits go through the same kind of validation the App Schema
 * itself is validated with, just scoped to one record's data instead of the
 * schema definition.
 */
export function buildRecordSchema(entity: Entity, viewFields: ViewFieldRef[]) {
  const shape: Record<string, ZodTypeAny> = {};
  for (const ref of viewFields) {
    const field = entity.fields.find((f) => f.id === ref.field);
    if (!field || field.type === "reference") continue;
    shape[field.id] = applyRequired(baseTypeSchema(field), field);
  }
  return z.object(shape);
}

export type RecordFormValues = Record<string, string | number | boolean>;
