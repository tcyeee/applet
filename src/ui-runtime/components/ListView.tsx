import type { AppDefinition, View, ViewFieldRef } from "@/app-schema/types";
import type { AppRouter } from "../router";
import { useEntityRecords } from "../useEntityRecords";
import { isVisible } from "../visibleWhen";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { FieldValue } from "./FieldValue";
import { EmptyState } from "./EmptyState";
import { ErrorState } from "./ErrorState";
import { LoadingState } from "./LoadingState";

export function ListView({
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

  const detailView = definition.views.find((v) => v.type === "detail" && v.entityId === view.entityId);
  const formView = definition.views.find((v) => v.type === "form" && v.entityId === view.entityId);

  if (!entity)
    return (
      <ErrorState
        message={`未找到实体 "${view.entityId}"`}
        filePath="src/ui-runtime/components/ListView.tsx"
        detail={{ appId, viewId: view.id }}
      />
    );
  if (loading) return <LoadingState />;
  if (error)
    return (
      <ErrorState
        message={error}
        filePath="src/ui-runtime/components/ListView.tsx"
        detail={{ appId, viewId: view.id, entityId: view.entityId }}
        onRetry={refetch}
      />
    );

  const columns: ViewFieldRef[] = (
    view.fields ?? entity.fields.map((f): ViewFieldRef => ({ field: f.id }))
  ).filter((ref) => entity.fields.some((f) => f.id === ref.field));

  return (
    <div className="flex flex-col gap-4">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold">{view.title ?? entity.name}</h2>
        {formView && (
          <Button size="sm" onClick={() => router.push(formView.id)}>
            新增
          </Button>
        )}
      </div>

      {records.length === 0 ? (
        <EmptyState
          message="暂无数据"
          actionLabel={formView ? "新增一条" : undefined}
          onAction={formView ? () => router.push(formView.id) : undefined}
        />
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              {columns.map((ref) => {
                const field = entity.fields.find((f) => f.id === ref.field)!;
                return <TableHead key={ref.field}>{field.label ?? field.id}</TableHead>;
              })}
              <TableHead className="text-right">操作</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {records.map((record) => (
              <TableRow
                key={record.id}
                className={detailView ? "cursor-pointer" : undefined}
                onClick={() => detailView && router.push(detailView.id, record.id)}
              >
                {columns.map((ref) => {
                  const field = entity.fields.find((f) => f.id === ref.field)!;
                  return (
                    <TableCell key={ref.field}>
                      {isVisible(record, ref.visibleWhen) ? (
                        <FieldValue field={field} value={record[ref.field]} />
                      ) : (
                        <span className="text-muted-foreground">—</span>
                      )}
                    </TableCell>
                  );
                })}
                <TableCell className="text-right">
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={(e) => {
                      e.stopPropagation();
                      void remove(record.id);
                    }}
                  >
                    删除
                  </Button>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
    </div>
  );
}
