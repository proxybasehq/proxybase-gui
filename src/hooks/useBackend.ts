import { useState, useCallback } from "react";

const STORAGE_KEY = "proxybase_backend_url";
const DEFAULT_BACKEND = import.meta.env.DEV
  ? "http://localhost:8080"
  : "https://api.proxybase.xyz";

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
