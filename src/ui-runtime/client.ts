import { invoke } from "@tauri-apps/api/core";
import type { AppDefinition } from "@/app-schema/types";

export interface AppRecord {
  id: string;
  name: string;
  description?: string;
  schemaVersion: string;
  definition: AppDefinition;
  status: "stopped" | "running";
  revision: number;
  installedAt: string;
  updatedAt: string;
}

export type RecordValues = Record<string, unknown>;
export type AppRuntimeRecord = RecordValues & {
  id: string;
  createdAt: string;
  updatedAt: string;
};

export interface RuntimeInfo {
  dataDir: string;
  version: string;
}

export const client = {
  listApps: () => invoke<AppRecord[]>("list_apps"),
  getApp: (appId: string) => invoke<AppRecord>("get_app", { appId }),
  installApp: (definition: AppDefinition) => invoke<AppRecord>("install_app", { definition }),
  updateApp: (appId: string, definition: AppDefinition, force = false) =>
    invoke<AppRecord>("update_app", { appId, definition, force }),
  uninstallApp: (appId: string, purgeData = false) =>
    invoke<void>("uninstall_app", { appId, purgeData }),

  getRuntimeInfo: () => invoke<RuntimeInfo>("get_runtime_info"),
  isOnboardingComplete: () => invoke<boolean>("is_onboarding_complete"),
  completeOnboarding: () => invoke<void>("complete_onboarding"),

  listRecords: (appId: string, entityId: string) =>
    invoke<AppRuntimeRecord[]>("list_records", { appId, entityId }),
  createRecord: (appId: string, entityId: string, values: RecordValues) =>
    invoke<AppRuntimeRecord>("create_record", { appId, entityId, values }),
  updateRecord: (appId: string, entityId: string, recordId: string, values: RecordValues) =>
    invoke<AppRuntimeRecord>("update_record", { appId, entityId, recordId, values }),
  deleteRecord: (appId: string, entityId: string, recordId: string) =>
    invoke<void>("delete_record", { appId, entityId, recordId }),
};

export type Client = typeof client;
