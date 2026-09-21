import { describe, expect, it } from "vitest";
import { aggregateRecords } from "../aggregate";
import type { AppRuntimeRecord } from "../client";

function record(overrides: Partial<AppRuntimeRecord>): AppRuntimeRecord {
  return { id: "1", createdAt: "now", updatedAt: "now", ...overrides };
}

describe("aggregateRecords", () => {
  const records = [
    record({ id: "1", category: "food", amount: 10 }),
    record({ id: "2", category: "food", amount: 20 }),
    record({ id: "3", category: "transport", amount: 5 }),
  ];

  it("sums a metric field per group", () => {
    const result = aggregateRecords(records, { groupBy: "category", aggregate: "sum", metricField: "amount" });
    expect(result).toEqual(
      expect.arrayContaining([
        { label: "food", value: 30 },
        { label: "transport", value: 5 },
      ]),
    );
  });

  it("counts records per group when aggregate is count", () => {
    const result = aggregateRecords(records, { groupBy: "category", aggregate: "count" });
    expect(result).toEqual(
      expect.arrayContaining([
        { label: "food", value: 2 },
        { label: "transport", value: 1 },
      ]),
    );
  });

  it("computes avg/min/max per group", () => {
    const avg = aggregateRecords(records, { groupBy: "category", aggregate: "avg", metricField: "amount" });
    expect(avg.find((p) => p.label === "food")?.value).toBe(15);

    const min = aggregateRecords(records, { groupBy: "category", aggregate: "min", metricField: "amount" });
    expect(min.find((p) => p.label === "food")?.value).toBe(10);

    const max = aggregateRecords(records, { groupBy: "category", aggregate: "max", metricField: "amount" });
    expect(max.find((p) => p.label === "food")?.value).toBe(20);
  });

  it("returns an empty array for no records", () => {
    expect(aggregateRecords([], { groupBy: "category", aggregate: "sum", metricField: "amount" })).toEqual([]);
  });
});
