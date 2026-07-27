import { useState, useCallback } from "react";

const STORAGE_KEY = "proxybase_backend_url";

function getDefaultBackend(): string {
  if (!import.meta.env.DEV) {
    return "https://api.proxybase.xyz";
  }
  return "http://localhost:8080";
}

const DEFAULT_BACKEND = getDefaultBackend();

export function useBackend() {
  const [backendUrl, setBackendUrl] = useState<string>(() => {
    return localStorage.getItem(STORAGE_KEY) || DEFAULT_BACKEND;
  });

  const updateBackend = useCallback((url: string) => {
    setBackendUrl(url);
    localStorage.setItem(STORAGE_KEY, url);
  }, []);

  return { backendUrl, updateBackend, defaultBackend: DEFAULT_BACKEND };
}
