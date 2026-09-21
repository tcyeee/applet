import type { VisibleWhen } from "@/app-schema/types";

/** Evaluates a `ViewFieldRef.visibleWhen` condition against one record. */
export function isVisible(record: Record<string, unknown>, condition: VisibleWhen | undefined): boolean {
  if (!condition) return true;
  const actual = record[condition.field];
  const expected = condition.value;
  switch (condition.operator) {
    case "eq":
      return actual === expected;
    case "neq":
      return actual !== expected;
    case "gt":
      return Number(actual) > Number(expected);
    case "gte":
      return Number(actual) >= Number(expected);
    case "lt":
      return Number(actual) < Number(expected);
    case "lte":
      return Number(actual) <= Number(expected);
  }
}
