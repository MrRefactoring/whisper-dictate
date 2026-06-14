import { useCallback, useEffect, useState } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export interface UpdaterState {
  available: boolean;
  version: string | null;
  installing: boolean;
  error: string | null;
  checking: boolean;
  upToDate: boolean;
}

export interface UpdaterActions {
  install: () => Promise<void>;
  dismiss: () => void;
  checkNow: () => Promise<void>;
}

export function useUpdater(): UpdaterState & UpdaterActions {
  const [available, setAvailable] = useState(false);
  const [version, setVersion] = useState<string | null>(null);
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [update, setUpdate] = useState<Update | null>(null);
  const [checking, setChecking] = useState(false);
  const [upToDate, setUpToDate] = useState(false);

  useEffect(() => {
    check()
      .then((u) => {
        if (u) {
          setUpdate(u);
          setAvailable(true);
          setVersion(u.version);
        }
      })
      .catch(() => {});
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

  const checkNow = useCallback(async () => {
    if (checking) return;
    setChecking(true);
    setUpToDate(false);
    try {
      const u = await check();
      if (u) {
        setUpdate(u);
        setAvailable(true);
        setVersion(u.version);
      } else {
        setUpToDate(true);
        setTimeout(() => setUpToDate(false), 3000);
      }
    } catch {
    } finally {
      setChecking(false);
    }
  }, [checking]);

  return { available, version, installing, error, checking, upToDate, install, dismiss, checkNow };
}
