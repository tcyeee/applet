import type { Field } from "@/app-schema/types";

function todayDateString(): string {
  const now = new Date();
  const yyyy = now.getFullYear();
  const mm = String(now.getMonth() + 1).padStart(2, "0");
  const dd = String(now.getDate()).padStart(2, "0");
  return `${yyyy}-${mm}-${dd}`;
}

/** Values that mean "nothing was stored here" — including the literal string
 * "null", which can end up in a record when a value was stringified before
 * being saved (e.g. by an MCP caller) instead of left absent. */
function isEmpty(value: unknown): boolean {
  return value === null || value === undefined || value === "" || value === "null";
}

/** Read-only rendering of one field's value, shared by ListView and DetailView. */
export function FieldValue({ field, value }: { field: Field; value: unknown }) {
  if (field.type === "date" && isEmpty(value)) {
    return <span>{todayDateString()}</span>;
  }
  if (isEmpty(value)) {
    return <span className="text-muted-foreground">—</span>;
  }
  if (field.type === "boolean") {
    return <span>{value ? "是" : "否"}</span>;
  }
  if (field.type === "date") {
    return <span>{String(value).slice(0, 10)}</span>;
  }
  if (field.type === "datetime") {
    return <span>{new Date(String(value)).toLocaleString()}</span>;
  }
  return <span>{String(value)}</span>;
}
