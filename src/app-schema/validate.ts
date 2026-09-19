import { appDefinitionSchema } from "./schema";
import type { AppDefinition } from "./types";

export type ValidationResult =
  | { success: true; data: AppDefinition }
  | { success: false; errors: string[] };

export function validateAppDefinition(input: unknown): ValidationResult {
  const result = appDefinitionSchema.safeParse(input);
  if (result.success) {
    return { success: true, data: result.data as AppDefinition };
  }
  const errors = result.error.issues.map(
    (issue) => `${issue.path.join(".") || "(root)"}: ${issue.message}`,
  );
  return { success: false, errors };
}
