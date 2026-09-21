// Bundles the App Schema validator into a single self-contained ESM file
// (`dist-cli/validate-app.mjs`) that the MCP server (a Rust process, see
// `src-tauri/src/mcp/validator.rs`) spawns via plain `node` to validate App
// definitions before trusting them. See `src/app-schema/validate-cli.ts` for
// the stdin/stdout contract.
//
// Deliberately calls esbuild's JS API instead of the `esbuild` CLI shim:
// pnpm's generated `node_modules/.bin/esbuild` wrapper always execs its
// target through `node`, but esbuild's postinstall replaces `bin/esbuild`
// with the real native binary — `node <native binary>` then fails to parse
// it as JavaScript. The JS API resolves and runs the native binary directly.
import * as esbuild from "esbuild";

await esbuild.build({
  entryPoints: ["src/app-schema/validate-cli.ts"],
  bundle: true,
  platform: "node",
  format: "esm",
  outfile: "dist-cli/validate-app.mjs",
});
