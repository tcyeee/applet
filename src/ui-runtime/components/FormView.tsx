import { useEffect, useMemo } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import type { AppDefinition, Field, View, ViewFieldRef } from "@/app-schema/types";
import type { AppRouter } from "../router";
import { useEntityRecords } from "../useEntityRecords";
import { buildRecordSchema } from "../fieldSchema";
import { evaluateExpression } from "../computeExpression";
import { isVisible } from "../visibleWhen";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { FieldInput } from "./FieldInput";
import { ErrorState } from "./ErrorState";
import { LoadingState } from "./LoadingState";

function todayDateString(): string {
  const now = new Date();
  const yyyy = now.getFullYear();
  const mm = String(now.getMonth() + 1).padStart(2, "0");
  const dd = String(now.getDate()).padStart(2, "0");
  return `${yyyy}-${mm}-${dd}`;
}

function defaultValueFor(field: Field): string | number | boolean {
  if (field.type === "boolean") return Boolean(field.default);
  if (field.type === "number") return typeof field.default === "number" ? field.default : "";
  if (field.type === "reference") return "";
  if (field.type === "date" && field.default === undefined) return todayDateString();
  return field.default !== undefined ? String(field.default) : "";
}

/** Values that mean "nothing was stored here" — including the literal string
 * "null", which can end up in a record when a value was stringified before
 * being saved (e.g. by an MCP caller) instead of left absent. */
function isEmptyValue(value: unknown): boolean {
  return value === null || value === undefined || value === "" || value === "null";
}

/** Same as `defaultValueFor`, but seeded from an existing record's stored
 * value when editing, so a bad/missing value (null, "", the literal string
 * "null") is repaired instead of round-tripped back into the record. */
function editValueFor(field: Field, stored: unknown): string | number | boolean {
  if (isEmptyValue(stored)) return defaultValueFor(field);
  if (field.type === "boolean") return Boolean(stored);
  return String(stored);
}

function fieldIdsInExpression(expression: string): string[] {
  return Array.from(new Set(expression.match(/[a-zA-Z][a-zA-Z0-9]*/g) ?? []));
}

export function FormView({
  appId,
  definition,
  view,
  router,
}: {
  appId: string;
  definition: AppDefinition;
  view: View;
  router: AppRouter;
}) {
  const entity = definition.dataModel.entities.find((e) => e.id === view.entityId);
  const { records, loading, error, refetch, create, update } = useEntityRecords(appId, view.entityId);
  const recordId = router.current.recordId;
  const editingRecord = recordId ? records.find((r) => r.id === recordId) : undefined;

  const fields: ViewFieldRef[] = useMemo(
    () =>
      (view.fields ?? entity?.fields.map((f): ViewFieldRef => ({ field: f.id })) ?? []).filter(
        (ref) => entity?.fields.some((f) => f.id === ref.field),
      ),
    [view.fields, entity],
  );

  const schema = useMemo(() => (entity ? buildRecordSchema(entity, fields) : undefined), [entity, fields]);
  const computeActions = definition.actions.filter(
    (a) => a.type === "compute" && a.entityId === view.entityId && a.expression && a.targetField,
  );

  const form = useForm({
    resolver: schema ? zodResolver(schema) : undefined,
    defaultValues: Object.fromEntries(
      fields.map((ref) => {
        const field = entity?.fields.find((f) => f.id === ref.field);
        return [ref.field, field ? defaultValueFor(field) : ""];
      }),
    ),
  });

  useEffect(() => {
    if (editingRecord) {
      form.reset(
        Object.fromEntries(
          fields.map((ref) => {
            const field = entity?.fields.find((f) => f.id === ref.field);
            return [ref.field, field ? editValueFor(field, editingRecord[ref.field]) : ""];
          }),
        ),
      );
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [editingRecord?.id]);

  const watched = form.watch();

  useEffect(() => {
    for (const action of computeActions) {
      const dependents = fieldIdsInExpression(action.expression!).filter((id) => id !== action.targetField);
      const values = Object.fromEntries(dependents.map((id) => [id, Number(watched[id]) || 0]));
      try {
        const result = evaluateExpression(action.expression!, values);
        if (watched[action.targetField!] !== result) {
          form.setValue(action.targetField!, result);
        }
      } catch {
        // invalid intermediate state (e.g. empty inputs mid-typing) — leave targetField as-is
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [JSON.stringify(watched)]);

  if (!entity || !schema)
    return (
      <ErrorState
        message={`未找到实体 "${view.entityId}"`}
        filePath="src/ui-runtime/components/FormView.tsx"
        detail={{ appId, viewId: view.id }}
      />
    );
  if (loading) return <LoadingState />;
  if (error)
    return (
      <ErrorState
        message={error}
        filePath="src/ui-runtime/components/FormView.tsx"
        detail={{ appId, viewId: view.id, entityId: view.entityId }}
        onRetry={refetch}
      />
    );

  const computedFieldIds = new Set(computeActions.map((a) => a.targetField));

  const onSubmit = form.handleSubmit(async (values) => {
    if (editingRecord) {
      await update(editingRecord.id, values);
    } else {
      await create(values);
    }
    router.pop();
  });

  return (
    <form onSubmit={onSubmit} className="flex max-w-md flex-col gap-4">
      <h2 className="text-lg font-semibold">{view.title ?? entity.name}</h2>
      {fields.map((ref) => {
        const field = entity.fields.find((f) => f.id === ref.field)!;
        if (!isVisible(watched, ref.visibleWhen)) return null;
        const errorMessage = form.formState.errors[ref.field]?.message as string | undefined;
        return (
          <div key={ref.field} className="flex flex-col gap-1.5">
            <Label htmlFor={ref.field}>{field.label ?? field.id}</Label>
            <FieldInput
              field={field}
              name={ref.field}
              control={form.control}
              appId={appId}
              disabled={computedFieldIds.has(ref.field)}
            />
            {errorMessage && <p className="text-destructive text-xs">{errorMessage}</p>}
          </div>
        );
      })}
      <div className="flex gap-2">
        <Button type="submit">保存</Button>
        <Button type="button" variant="outline" onClick={() => router.pop()}>
          取消
        </Button>
      </div>
    </form>
  );
}
