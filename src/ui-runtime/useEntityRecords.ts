import { useCallback, useEffect, useState } from "react";
import { client, type AppRuntimeRecord, type RecordValues } from "./client";

export interface EntityRecordsState {
  records: AppRuntimeRecord[];
  loading: boolean;
  error: string | null;
  refetch: () => void;
  create: (values: RecordValues) => Promise<AppRuntimeRecord>;
  update: (recordId: string, values: RecordValues) => Promise<AppRuntimeRecord>;
  remove: (recordId: string) => Promise<void>;
}

function errorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

/** Loads and mutates one entity's records for an installed App, via the Tauri CRUD commands. */
export function useEntityRecords(appId: string, entityId: string): EntityRecordsState {
  const [records, setRecords] = useState<AppRuntimeRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [reloadToken, setReloadToken] = useState(0);

  useEffect(() => {
    let cancelled = false;
    async function run() {
      setLoading(true);
      setError(null);
      try {
        const data = await client.listRecords(appId, entityId);
        if (!cancelled) setRecords(data);
      } catch (err) {
        if (!cancelled) setError(errorMessage(err));
      } finally {
        if (!cancelled) setLoading(false);
      }
    }
    void run();
    return () => {
      cancelled = true;
    };
  }, [appId, entityId, reloadToken]);

  const refetch = useCallback(() => setReloadToken((t) => t + 1), []);

  const create = useCallback(
    async (values: RecordValues) => {
      const created = await client.createRecord(appId, entityId, values);
      refetch();
      return created;
    },
    [appId, entityId, refetch],
  );

  const update = useCallback(
    async (recordId: string, values: RecordValues) => {
      const updated = await client.updateRecord(appId, entityId, recordId, values);
      refetch();
      return updated;
    },
    [appId, entityId, refetch],
  );

  const remove = useCallback(
    async (recordId: string) => {
      await client.deleteRecord(appId, entityId, recordId);
      refetch();
    },
    [appId, entityId, refetch],
  );

  return { records, loading, error, refetch, create, update, remove };
}
