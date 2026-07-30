import { useState, useEffect, useCallback } from "react";
import { check } from "@tauri-apps/plugin-updater";
import type { DownloadEvent } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

interface UpdateProgress {
  downloaded: number;
  contentLength: number;
  percentage: number;
}

interface UpdateManifest {
  version: string;
  body?: string;
}

export function useUpdater() {
  const [update, setUpdate] = useState<UpdateManifest | null>(null);
  const [checking, setChecking] = useState(false);
  const [downloading, setDownloading] = useState(false);
  const [progress, setProgress] = useState<UpdateProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [readyToRestart, setReadyToRestart] = useState(false);

  const checkForUpdates = useCallback(async (silent = false) => {
    if (checking || downloading) return;
    setChecking(true);
    setError(null);
    try {
      const result = await check();
      if (result?.available) {
        setUpdate({
          version: result.version,
          body: result.body || undefined,
        });
      } else {
        setUpdate(null);
      }
    } catch (err) {
      if (!silent) setError(String(err));
    } finally {
      setChecking(false);
    }
  }, [checking, downloading]);

  const downloadAndInstall = useCallback(async () => {
    if (!update) return;
    setDownloading(true);
    setError(null);
    try {
      const result = await check();
      if (!result?.available) return;

      let downloadedBytes = 0;
      await result.downloadAndInstall((event: DownloadEvent) => {
        switch (event.event) {
          case "Started":
            setProgress({
              downloaded: 0,
              contentLength: event.data.contentLength || 0,
              percentage: 0,
            });
            break;
          case "Progress":
            downloadedBytes += event.data.chunkLength;
            setProgress((prev) => ({
              downloaded: downloadedBytes,
              contentLength: prev?.contentLength || 1,
              percentage: Math.min(
                99,
                Math.round((downloadedBytes / (prev?.contentLength || 1)) * 100)
              ),
            }));
            break;
          case "Finished":
            setDownloading(false);
            setProgress(null);
            setReadyToRestart(true);
            break;
        }
      });
    } catch (err) {
      setError(String(err));
      setDownloading(false);
    }
  }, [update]);

  const restartApp = useCallback(async () => {
    await relaunch();
  }, []);

  // Check on mount + every 6 hours
  useEffect(() => {
    checkForUpdates(true);
    const interval = setInterval(
      () => checkForUpdates(true),
      6 * 60 * 60 * 1000
    );
    return () => clearInterval(interval);
  }, []);

  return {
    update,
    checking,
    downloading,
    progress,
    error,
    readyToRestart,
    checkForUpdates,
    downloadAndInstall,
    restartApp,
  };
}
