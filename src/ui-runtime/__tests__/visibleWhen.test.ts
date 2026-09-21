import { describe, expect, it } from "vitest";
import { isVisible } from "../visibleWhen";

describe("isVisible", () => {
  it("returns true when there is no condition", () => {
    expect(isVisible({ monthlyLimit: 0 }, undefined)).toBe(true);
  });

  it("evaluates comparison operators", () => {
    const record = { monthlyLimit: 100 };
    expect(isVisible(record, { field: "monthlyLimit", operator: "gt", value: 0 })).toBe(true);
    expect(isVisible(record, { field: "monthlyLimit", operator: "gt", value: 100 })).toBe(false);
    expect(isVisible(record, { field: "monthlyLimit", operator: "eq", value: 100 })).toBe(true);
    expect(isVisible(record, { field: "monthlyLimit", operator: "neq", value: 100 })).toBe(false);
    expect(isVisible(record, { field: "monthlyLimit", operator: "lte", value: 100 })).toBe(true);
    expect(isVisible(record, { field: "monthlyLimit", operator: "lt", value: 100 })).toBe(false);
  });
});
