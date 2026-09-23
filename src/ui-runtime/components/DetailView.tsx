import type { AppDefinition, View, ViewFieldRef } from "@/app-schema/types";
import type { AppRouter } from "../router";
import { useEntityRecords } from "../useEntityRecords";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { FieldValue } from "./FieldValue";
import { EmptyState } from "./EmptyState";
import { ErrorState } from "./ErrorState";
import { LoadingState } from "./LoadingState";

export function DetailView({
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
  const { records, loading, error, refetch, remove } = useEntityRecords(appId, view.entityId);
  const formView = definition.views.find((v) => v.type === "form" && v.entityId === view.entityId);

  if (!entity)
    return (
      <ErrorState
        message={`未找到实体 "${view.entityId}"`}
        filePath="src/ui-runtime/components/DetailView.tsx"
        detail={{ appId, viewId: view.id }}
      />
    );
  if (loading) return <LoadingState />;
  if (error)
    return (
      <ErrorState
        message={error}
        filePath="src/ui-runtime/components/DetailView.tsx"
        detail={{ appId, viewId: view.id, entityId: view.entityId }}
        onRetry={refetch}
      />
    );

  const record = records.find((r) => r.id === router.current.recordId);
  if (!record) return <EmptyState message="未找到该记录，可能已被删除" />;

  const fields: ViewFieldRef[] = (
    view.fields ?? entity.fields.map((f): ViewFieldRef => ({ field: f.id }))
  ).filter((ref) => entity.fields.some((f) => f.id === ref.field));

  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between">
        <CardTitle>{view.title ?? entity.name}</CardTitle>
        <div className="flex gap-2">
          {formView && (
            <Button size="sm" variant="outline" onClick={() => router.push(formView.id, record.id)}>
              编辑
            </Button>
          )}
          <Button
            size="sm"
            variant="destructive"
            onClick={async () => {
              await remove(record.id);
              router.pop();
            }}
          >
            删除
          </Button>
        </div>
      </CardHeader>
      <CardContent className="grid grid-cols-2 gap-4">
        {fields.map((ref) => {
          const field = entity.fields.find((f) => f.id === ref.field)!;
          return (
            <div key={ref.field} className="flex flex-col gap-1">
              <span className="text-muted-foreground text-xs">{field.label ?? field.id}</span>
              <FieldValue field={field} value={record[ref.field]} />
            </div>
          );
        })}
      </CardContent>
    </Card>
  );
}
