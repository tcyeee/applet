import { describe, expect, it } from "vitest";
import { validateAppDefinition } from "../validate";
import bookkeeping from "../examples/bookkeeping.json";
import habitTracker from "../examples/habit-tracker.json";

describe("validateAppDefinition - example apps", () => {
  it("accepts the bookkeeping example", () => {
    const result = validateAppDefinition(bookkeeping);
    expect(result.success).toBe(true);
  });

  it("accepts the habit-tracker example", () => {
    const result = validateAppDefinition(habitTracker);
    expect(result.success).toBe(true);
  });
});

describe("validateAppDefinition - invalid input", () => {
  const minimalValid = {
    schemaVersion: "1",
    id: "demo-app",
    name: "Demo",
    dataModel: {
      entities: [
        { id: "item", name: "Item", fields: [{ id: "title", type: "string", required: true }] },
      ],
    },
    views: [],
    actions: [],
    automations: [],
  };

  it("rejects a missing schemaVersion", () => {
    const rest: Record<string, unknown> = { ...minimalValid };
    delete rest.schemaVersion;
    const result = validateAppDefinition(rest);
    expect(result.success).toBe(false);
  });

  it("rejects an unknown schemaVersion", () => {
    const result = validateAppDefinition({ ...minimalValid, schemaVersion: "2" });
    expect(result.success).toBe(false);
  });

  it("rejects an entity with no fields", () => {
    const result = validateAppDefinition({
      ...minimalValid,
      dataModel: { entities: [{ id: "item", name: "Item", fields: [] }] },
    });
    expect(result.success).toBe(false);
  });

  it("rejects a duplicate entity id", () => {
    const entity = { id: "item", name: "Item", fields: [{ id: "title", type: "string" }] };
    const result = validateAppDefinition({
      ...minimalValid,
      dataModel: { entities: [entity, entity] },
    });
    expect(result.success).toBe(false);
    if (!result.success) {
      expect(result.errors.some((e) => e.includes("duplicate entity id"))).toBe(true);
    }
  });

  it("rejects an invalid field type", () => {
    const result = validateAppDefinition({
      ...minimalValid,
      dataModel: {
        entities: [{ id: "item", name: "Item", fields: [{ id: "title", type: "notARealType" }] }],
      },
    });
    expect(result.success).toBe(false);
  });

  it("rejects a view referencing an unknown entity", () => {
    const result = validateAppDefinition({
      ...minimalValid,
      views: [{ id: "itemList", type: "list", entityId: "doesNotExist" }],
    });
    expect(result.success).toBe(false);
    if (!result.success) {
      expect(result.errors.some((e) => e.includes("unknown entity"))).toBe(true);
    }
  });

  it("rejects a kebab-case field id", () => {
    const result = validateAppDefinition({
      ...minimalValid,
      dataModel: {
        entities: [{ id: "item", name: "Item", fields: [{ id: "not-camel-case", type: "string" }] }],
      },
    });
    expect(result.success).toBe(false);
  });
});
