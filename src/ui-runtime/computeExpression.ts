/**
 * Safe evaluator for `Action.expression` ("compute" actions), e.g.
 * "unitPrice * quantity". Only numbers, `+ - * / ( )`, whitespace, and
 * identifiers referencing the entity's own fields are accepted — this is
 * deliberately not a general expression language, per the App Schema's
 * `expression` field being a "restricted arithmetic expression".
 */

const TOKEN_PATTERN = /\s*(?:([0-9]+(?:\.[0-9]+)?)|([a-zA-Z][a-zA-Z0-9]*)|([+\-*/()]))/y;

type Token = { kind: "number"; value: number } | { kind: "ident"; value: string } | { kind: "op"; value: string };

function tokenize(expression: string): Token[] {
  const tokens: Token[] = [];
  TOKEN_PATTERN.lastIndex = 0;
  let index = 0;
  while (index < expression.length) {
    TOKEN_PATTERN.lastIndex = index;
    const match = TOKEN_PATTERN.exec(expression);
    if (!match || match[0].length === 0) {
      throw new Error(`invalid character in expression at position ${index}`);
    }
    if (match[1] !== undefined) tokens.push({ kind: "number", value: Number(match[1]) });
    else if (match[2] !== undefined) tokens.push({ kind: "ident", value: match[2] });
    else if (match[3] !== undefined) tokens.push({ kind: "op", value: match[3] });
    index = TOKEN_PATTERN.lastIndex;
  }
  return tokens;
}

class Parser {
  private pos = 0;
  constructor(
    private tokens: Token[],
    private values: Record<string, number>,
  ) {}

  private peek() {
    return this.tokens[this.pos];
  }

  private next() {
    return this.tokens[this.pos++];
  }

  parse(): number {
    const value = this.parseExpr();
    if (this.pos !== this.tokens.length) {
      throw new Error("unexpected trailing input in expression");
    }
    return value;
  }

  private parseExpr(): number {
    let value = this.parseTerm();
    while (this.peek()?.kind === "op" && (this.peek().value === "+" || this.peek().value === "-")) {
      const op = this.next() as Token & { kind: "op" };
      const rhs = this.parseTerm();
      value = op.value === "+" ? value + rhs : value - rhs;
    }
    return value;
  }

  private parseTerm(): number {
    let value = this.parseFactor();
    while (this.peek()?.kind === "op" && (this.peek().value === "*" || this.peek().value === "/")) {
      const op = this.next() as Token & { kind: "op" };
      const rhs = this.parseFactor();
      value = op.value === "*" ? value * rhs : value / rhs;
    }
    return value;
  }

  private parseFactor(): number {
    const token = this.next();
    if (!token) throw new Error("unexpected end of expression");
    if (token.kind === "number") return token.value;
    if (token.kind === "ident") {
      if (!(token.value in this.values)) {
        throw new Error(`unknown field "${token.value}" in expression`);
      }
      return this.values[token.value];
    }
    if (token.kind === "op" && token.value === "(") {
      const value = this.parseExpr();
      const close = this.next();
      if (!close || close.kind !== "op" || close.value !== ")") {
        throw new Error("mismatched parentheses in expression");
      }
      return value;
    }
    if (token.kind === "op" && token.value === "-") {
      return -this.parseFactor();
    }
    throw new Error("unexpected token in expression");
  }
}

/** Evaluates a restricted arithmetic expression against a record's field values. */
export function evaluateExpression(expression: string, values: Record<string, number>): number {
  const tokens = tokenize(expression);
  return new Parser(tokens, values).parse();
}
