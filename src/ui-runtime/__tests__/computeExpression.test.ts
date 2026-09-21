import { describe, expect, it } from "vitest";
import { evaluateExpression } from "../computeExpression";

describe("evaluateExpression", () => {
  it("evaluates a simple multiplication", () => {
    expect(evaluateExpression("unitPrice * quantity", { unitPrice: 3, quantity: 4 })).toBe(12);
  });

  it("respects operator precedence and parentheses", () => {
    expect(evaluateExpression("(a + b) * c", { a: 1, b: 2, c: 3 })).toBe(9);
    expect(evaluateExpression("a + b * c", { a: 1, b: 2, c: 3 })).toBe(7);
  });

  it("supports unary minus", () => {
    expect(evaluateExpression("-a + 5", { a: 2 })).toBe(3);
  });

  it("throws on unknown field references", () => {
    expect(() => evaluateExpression("unknownField * 2", {})).toThrow(/unknown field/);
  });

  it("throws on disallowed characters", () => {
    expect(() => evaluateExpression("a; DROP TABLE apps;--", { a: 1 })).toThrow();
  });

  it("throws on mismatched parentheses", () => {
    expect(() => evaluateExpression("(a + 1", { a: 1 })).toThrow();
  });
});
