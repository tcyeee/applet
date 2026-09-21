import type { ChartConfig } from "@/app-schema/types";
import type { AppRuntimeRecord } from "./client";

export interface AggregatedPoint {
  label: string;
  value: number;
}

/**
 * Groups records by `chart.groupBy` and aggregates `chart.metricField` (or
 * record count, for `aggregate: "count"`) per group. Runs client-side since
 * there's no SQL aggregation Tauri command yet and App data sets are small.
 */
export function aggregateRecords(
  records: AppRuntimeRecord[],
  chart: ChartConfig,
): AggregatedPoint[] {
  const groups = new Map<string, number[]>();
  for (const record of records) {
    const label = String(record[chart.groupBy] ?? "");
    const metric = chart.metricField ? Number(record[chart.metricField] ?? 0) : 1;
    const bucket = groups.get(label);
    if (bucket) bucket.push(metric);
    else groups.set(label, [metric]);
  }

  return Array.from(groups.entries()).map(([label, values]) => ({
    label,
    value: aggregate(chart.aggregate, values),
  }));
}

function aggregate(kind: ChartConfig["aggregate"], values: number[]): number {
  switch (kind) {
    case "sum":
      return values.reduce((a, b) => a + b, 0);
    case "count":
      return values.length;
    case "avg":
      return values.reduce((a, b) => a + b, 0) / values.length;
    case "min":
      return Math.min(...values);
    case "max":
      return Math.max(...values);
  }
}
