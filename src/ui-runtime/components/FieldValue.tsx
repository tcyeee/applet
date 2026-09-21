import type { Field } from "@/app-schema/types";

/** Read-only rendering of one field's value, shared by ListView and DetailView. */
export function FieldValue({ field, value }: { field: Field; value: unknown }) {
  if (value === null || value === undefined || value === "") {
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
