import { describe, expect, it } from "vitest";
import { buildRecordSchema } from "../fieldSchema";
import type { Entity } from "@/app-schema/types";

const entity: Entity = {
  id: "transaction",
  name: "交易记录",
  fields: [
    { id: "amount", type: "number", required: true, label: "金额" },
    { id: "note", type: "string" },
    { id: "category", type: "enum", options: ["food", "transport"], required: true },
    { id: "paid", type: "boolean" },
  ],
};

describe("buildRecordSchema", () => {
  const schema = buildRecordSchema(entity, [
    { field: "amount" },
    { field: "note" },
    { field: "category" },
    { field: "paid" },
  ]);

  it("accepts a fully valid record", () => {
    const result = schema.safeParse({ amount: 12.5, note: "lunch", category: "food", paid: true });
    expect(result.success).toBe(true);
  });

  it("rejects a missing required number field", () => {
    const result = schema.safeParse({ note: "lunch", category: "food" });
    expect(result.success).toBe(false);
  });

  it("rejects an enum value outside the declared options", () => {
    const result = schema.safeParse({ amount: 1, category: "not-an-option" });
    expect(result.success).toBe(false);
  });

  it("allows optional fields to be omitted", () => {
    const result = schema.safeParse({ amount: 1, category: "food" });
    expect(result.success).toBe(true);
  });

  it("skips fields the view doesn't reference", () => {
    const withoutCategory = buildRecordSchema(entity, [{ field: "amount" }]);
    const result = withoutCategory.safeParse({ amount: 1 });
    expect(result.success).toBe(true);
  });
});
