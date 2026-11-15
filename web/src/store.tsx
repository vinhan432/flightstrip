// One context for the whole page. It owns the loaded session and the
// selection; the playback clock lives in usePlayback and everything else
// derives from those two.

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import type { Session, Severity } from "./types";
import { parseSession } from "./utils/parseSession";

export interface SessionManifestEntry {
  id: string;
  title: string;
  hook: string;
  severity: Severity;
  file: string;
}

export type LoadStatus = "idle" | "loading" | "ready" | "error";

export interface Store {
  manifest: SessionManifestEntry[];
  session: Session | null;
  status: LoadStatus;
  error: string | null;
  /** The selected step index, or null. */
  selectedStep: number | null;
  selectStep: (index: number | null) => void;
  loadSession: (id: string) => Promise<void>;
  loadFromFile: (file: File) => Promise<void>;
  loadFromText: (text: string, label?: string) => void;
  toast: string | null;
  showToast: (message: string) => void;
}

const StoreContext = createContext<Store | null>(null);

const MANIFEST_URL = "sessions/index.json";
const DEFAULT_SESSION = "risky-session";

export function StoreProvider({ children }: { children: ReactNode }) {
  const [manifest, setManifest] = useState<SessionManifestEntry[]>([]);
  const [session, setSession] = useState<Session | null>(null);
  const [status, setStatus] = useState<LoadStatus>("idle");
  const [error, setError] = useState<string | null>(null);
  const [selectedStep, setSelectedStep] = useState<number | null>(null);
  const [toast, setToast] = useState<string | null>(null);
  const toastTimer = useRef<number | undefined>(undefined);

  const showToast = useCallback((message: string) => {
    setToast(message);
    window.clearTimeout(toastTimer.current);
    toastTimer.current = window.setTimeout(() => setToast(null), 3200);
  }, []);

  const loadFromText = useCallback(
    (text: string, label = "session") => {
      let parsed: unknown;
      try {
        parsed = JSON.parse(text);
      } catch {
        setStatus("error");
        setError("Not valid JSON");
        return;
      }
      const result = parseSession(parsed);
      if (!result.ok) {
        setStatus("error");
        setError(result.error);
        return;
      }
      setSession(result.session);
      setStatus("ready");
      setError(null);
      setSelectedStep(null);
      showToast(`Session loaded: ${result.session.meta.title || label}`);
    },
    [showToast],
  );

  const loadSession = useCallback(
    async (id: string) => {
      setStatus("loading");
      setError(null);
      try {
        const entry =
          manifest.find((m) => m.id === id) ??
          ({ file: `${id}.json` } as SessionManifestEntry);
        const response = await fetch(`sessions/${entry.file}`);
        if (!response.ok) {
          throw new Error(`HTTP ${response.status}`);
        }
        loadFromText(await response.text(), id);
      } catch (cause) {
        setStatus("error");
        setError(cause instanceof Error ? cause.message : "Failed to load");
      }
    },
    [manifest, loadFromText],
  );

  const loadFromFile = useCallback(
    async (file: File) => {
      setStatus("loading");
      setError(null);
      try {
        loadFromText(await file.text(), file.name);
      } catch (cause) {
        setStatus("error");
        setError(cause instanceof Error ? cause.message : "Failed to read file");
      }
    },
    [loadFromText],
  );

  // The catalogue, then the default session.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const response = await fetch(MANIFEST_URL);
        if (!response.ok) throw new Error(`HTTP ${response.status}`);
        const list = (await response.json()) as SessionManifestEntry[];
        if (!cancelled) setManifest(list);
      } catch {
        // A missing catalogue is not fatal: the Drop Zone still works.
        if (!cancelled) setManifest([]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (manifest.length === 0) return;
    void loadSession(DEFAULT_SESSION);
    // Only on the first catalogue load.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [manifest.length]);

  const value = useMemo<Store>(
    () => ({
      manifest,
      session,
      status,
      error,
      selectedStep,
      selectStep: setSelectedStep,
      loadSession,
      loadFromFile,
      loadFromText,
      toast,
      showToast,
    }),
    [
      manifest,
      session,
      status,
      error,
      selectedStep,
      loadSession,
      loadFromFile,
      loadFromText,
      toast,
      showToast,
    ],
  );

  return (
    <StoreContext.Provider value={value}>{children}</StoreContext.Provider>
  );
}

export function useStore(): Store {
  const store = useContext(StoreContext);
  if (!store) throw new Error("useStore must be used inside <StoreProvider>");
  return store;
}
