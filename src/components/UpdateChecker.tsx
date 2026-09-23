import { useState } from "react";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { Button } from "@/components/ui/button";

type Status = "idle" | "checking" | "up-to-date" | "downloading" | "error";

/** Runtime self-update (TODO step 6, "应用自更新机制") — distinct from an
 * App's own data migration (`src-tauri/src/runtime/appdb.rs`), this upgrades
 * the Applet binary itself via `tauri-plugin-updater` against the
 * `latest.json` the release workflow publishes to GitHub Releases. */
export function UpdateChecker() {
  const [status, setStatus] = useState<Status>("idle");
  const [error, setError] = useState<string | null>(null);

  const run = async () => {
    setStatus("checking");
    setError(null);
    try {
      const update = await check();
      if (!update) {
        setStatus("up-to-date");
        return;
      }
      setStatus("downloading");
      await update.downloadAndInstall();
      await relaunch();
    } catch (err) {
      const message = String(err);
      setError(
        message.includes("Could not fetch a valid release JSON")
          ? "尚未发布任何版本，暂时无法检查更新"
          : message,
      );
      setStatus("error");
    }
  };

  const label = {
    idle: "检查更新",
    checking: "检查中…",
    "up-to-date": "已是最新版本",
    downloading: "下载并安装中…",
    error: "检查失败，重试",
  }[status];

  return (
    <div className="flex items-center gap-2">
      <Button variant="ghost" size="sm" onClick={run} disabled={status === "checking" || status === "downloading"}>
        {label}
      </Button>
      {error && <span className="text-destructive text-xs">{error}</span>}
    </div>
  );
}
