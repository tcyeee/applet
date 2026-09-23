import { Bar, BarChart, CartesianGrid, ResponsiveContainer, Tooltip, XAxis, YAxis } from "recharts";
import type { AppDefinition, View } from "@/app-schema/types";
import { useEntityRecords } from "../useEntityRecords";
import { aggregateRecords } from "../aggregate";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { EmptyState } from "./EmptyState";
import { ErrorState } from "./ErrorState";
import { LoadingState } from "./LoadingState";

export function ChartView({
  appId,
  definition,
  view,
}: {
  appId: string;
  definition: AppDefinition;
  view: View;
}) {
  const entity = definition.dataModel.entities.find((e) => e.id === view.entityId);
  const { records, loading, error, refetch } = useEntityRecords(appId, view.entityId);

  if (!entity || !view.chart)
    return (
      <ErrorState
        message="图表配置不完整"
        filePath="src/ui-runtime/components/ChartView.tsx"
        detail={{ appId, viewId: view.id }}
      />
    );
  if (loading) return <LoadingState />;
  if (error)
    return (
      <ErrorState
        message={error}
        filePath="src/ui-runtime/components/ChartView.tsx"
        detail={{ appId, viewId: view.id, entityId: view.entityId }}
        onRetry={refetch}
      />
    );

  const data = aggregateRecords(records, view.chart);
  if (data.length === 0) return <EmptyState message="暂无数据可供统计" />;

  return (
    <Card>
      <CardHeader>
        <CardTitle>{view.title ?? entity.name}</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="h-80 w-full">
          <ResponsiveContainer width="100%" height="100%">
            <BarChart data={data}>
              <CartesianGrid strokeDasharray="3 3" />
              <XAxis dataKey="label" />
              <YAxis />
              <Tooltip />
              <Bar dataKey="value" fill="var(--color-primary)" radius={4} />
            </BarChart>
          </ResponsiveContainer>
        </div>
      </CardContent>
    </Card>
  );
}
