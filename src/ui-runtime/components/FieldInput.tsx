import { Controller, type Control } from "react-hook-form";
import type { Field } from "@/app-schema/types";
import { Input } from "@/components/ui/input";
import { Checkbox } from "@/components/ui/checkbox";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useEntityRecords } from "../useEntityRecords";

interface FieldInputProps {
  field: Field;
  name: string;
  control: Control;
  disabled?: boolean;
  appId: string;
}

function ReferenceSelect({
  field,
  name,
  control,
  disabled,
  appId,
}: FieldInputProps & { field: Extract<Field, { type: "reference" }> }) {
  const { records, loading } = useEntityRecords(appId, field.entityId);
  return (
    <Controller
      name={name}
      control={control}
      render={({ field: rhf }) => (
        <Select value={String(rhf.value ?? "")} onValueChange={rhf.onChange} disabled={disabled || loading}>
          <SelectTrigger className="w-full">
            <SelectValue placeholder={loading ? "加载中…" : "请选择"} />
          </SelectTrigger>
          <SelectContent>
            {records.map((r) => (
              <SelectItem key={r.id} value={r.id}>
                {r.id}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      )}
    />
  );
}

export function FieldInput(props: FieldInputProps) {
  const { field, name, control, disabled } = props;

  if (field.type === "boolean") {
    return (
      <Controller
        name={name}
        control={control}
        render={({ field: rhf }) => (
          <Checkbox checked={Boolean(rhf.value)} onCheckedChange={rhf.onChange} disabled={disabled} />
        )}
      />
    );
  }

  if (field.type === "enum") {
    return (
      <Controller
        name={name}
        control={control}
        render={({ field: rhf }) => (
          <Select value={String(rhf.value ?? "")} onValueChange={rhf.onChange} disabled={disabled}>
            <SelectTrigger className="w-full">
              <SelectValue placeholder="请选择" />
            </SelectTrigger>
            <SelectContent>
              {field.options.map((option) => (
                <SelectItem key={option} value={option}>
                  {option}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        )}
      />
    );
  }

  if (field.type === "reference" && field.cardinality === "one") {
    return <ReferenceSelect {...props} field={field} />;
  }

  const inputType = field.type === "date" ? "date" : field.type === "datetime" ? "datetime-local" : field.type === "number" ? "number" : "text";

  return (
    <Controller
      name={name}
      control={control}
      render={({ field: rhf }) => (
        <Input
          type={inputType}
          step={field.type === "number" ? "any" : undefined}
          value={rhf.value ?? ""}
          onChange={rhf.onChange}
          onBlur={rhf.onBlur}
          disabled={disabled}
        />
      )}
    />
  );
}
