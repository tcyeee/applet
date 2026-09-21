// Standalone entry point bundled to `dist-cli/validate-app.mjs` (see
// `pnpm build:mcp-validator`) so the MCP server (a Rust process, see
// `src-tauri/src/mcp/validator.rs`) can run App definitions through the one
// sanctioned validator (`validateAppDefinition`) without re-implementing Zod
// rules in Rust. See AGENTS.md's App Schema rules for why this must stay the
// only validation path.
//
// Contract: read the entire stdin as JSON (one AppDefinition candidate),
// write a single line of `{ success, data } | { success, errors }` JSON to
// stdout, always exit 0. Malformed stdin JSON is reported as a validation
// failure rather than a crash, so the Rust side only ever has to parse
// stdout — it never has to interpret an exit code.
//
// This is a Node CLI entry point, not browser/React code — it's excluded
// from the root tsconfig.json (which lacks @types/node) so `pnpm build`'s
// `tsc` step doesn't flag `process`/`Buffer` as undefined; esbuild (which
// only strips types, not checks them) bundles it fine regardless.
import { validateAppDefinition } from "./validate";

function readStdin(): Promise<string> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    process.stdin.on("data", (chunk) => chunks.push(chunk));
    process.stdin.on("end", () => resolve(Buffer.concat(chunks).toString("utf8")));
    process.stdin.on("error", reject);
  });
}

async function main() {
  const raw = await readStdin();
  let input: unknown;
  try {
    input = JSON.parse(raw);
  } catch (e) {
    process.stdout.write(
      JSON.stringify({ success: false, errors: [`invalid JSON: ${(e as Error).message}`] }),
    );
    return;
  }
  process.stdout.write(JSON.stringify(validateAppDefinition(input)));
}

main();
