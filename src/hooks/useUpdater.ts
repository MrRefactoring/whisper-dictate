import { useCallback, useEffect, useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export interface UpdaterState {
  available: boolean;
  version: string | null;
  installing: boolean;
  error: string | null;
}

export interface UpdaterActions {
  install: () => Promise<void>;
  dismiss: () => void;
}

export function useUpdater(): UpdaterState & UpdaterActions {
  const [available, setAvailable] = useState(false);
  const [version, setVersion] = useState<string | null>(null);
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [update, setUpdate] = useState<Update | null>(null);

  useEffect(() => {
    check()
      .then((u) => {
        if (u) {
          setUpdate(u);
          setAvailable(true);
          setVersion(u.version);
        }
      })
      .catch(() => {
        // Silent: no network or endpoint not configured — expected in dev.
      });
  }, []);

  const install = useCallback(async () => {
    if (!update) return;
    setInstalling(true);
    try {
      await update.downloadAndInstall();
      await relaunch();
    } catch (e) {
      setError(String(e));
      setInstalling(false);
    }
  }, [update]);

  const dismiss = useCallback(() => setAvailable(false), []);

  return { available, version, installing, error, install, dismiss };
}
