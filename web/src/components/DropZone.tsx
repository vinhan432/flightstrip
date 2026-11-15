// Load your own recording. Validates with the same parser the fixtures use,
// so a malformed file names the field rather than blanking the page.

import { useCallback, useRef, useState } from "react";
import type { Session } from "../types";
import { parseSession } from "../utils/parseSession";
import { readFileText } from "../utils/readFile";

interface Props {
  onLoad: (session: Session) => void;
  onError?: (message: string) => void;
}

export function DropZone({ onLoad, onError }: Props) {
  const [over, setOver] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [shake, setShake] = useState(false);
  const inputRef = useRef<HTMLInputElement | null>(null);

  const handleText = useCallback(
    (text: string) => {
      let parsed: unknown;
      try {
        parsed = JSON.parse(text);
      } catch {
        setError("Invalid session file: not valid JSON");
        onError?.("Invalid session file");
        return;
      }

      const result = parseSession(parsed);
      if (!result.ok) {
        setError(`Invalid session file: ${result.error}`);
        onError?.(`Invalid session file: ${result.error}`);
        return;
      }

      setError(null);
      onLoad(result.session);
    },
    [onLoad, onError],
  );

  const readFile = useCallback(
    async (file: File) => {
      try {
        handleText(await readFileText(file));
      } catch {
        setError("Invalid session file: could not read the file");
      }
    },
    [handleText],
  );

  const onDrop = useCallback(
    (event: React.DragEvent) => {
      event.preventDefault();
      setOver(false);
      const file = event.dataTransfer?.files?.[0];
      if (!file) {
        setError("Invalid session file: no file received");
        return;
      }
      void readFile(file);
    },
    [readFile],
  );

  // A failed drop shakes once, then settles.
  const failShake = useCallback(() => {
    setShake(true);
    window.setTimeout(() => setShake(false), 340);
  }, []);

  return (
    <div
      className={`dropzone${over ? " is-over" : ""}${error ? " is-error" : ""}${shake ? " shake" : ""}`}
      role="application"
      aria-label="Drop a session file here to load it"
      tabIndex={0}
      onClick={() => inputRef.current?.click()}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          inputRef.current?.click();
        }
      }}
      onDragOver={(event) => {
        event.preventDefault();
        setOver(true);
      }}
      onDragLeave={() => setOver(false)}
      onDrop={(event) => {
        onDrop(event);
        if (error) failShake();
      }}
    >
      <svg
        width="34"
        height="34"
        viewBox="0 0 24 24"
        fill="none"
        stroke="var(--ink-light)"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden="true"
      >
        <path d="M12 16V4" />
        <path d="m7 9 5-5 5 5" />
        <path d="M4 16v2a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-2" />
      </svg>

      <p className="mono" style={{ fontSize: "0.95rem" }}>
        {over ? "Release to load" : "Drop a session.json here"}
      </p>
      <p className="muted" style={{ fontSize: "0.78rem" }}>
        or click to choose a file
      </p>

      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}

      <input
        ref={inputRef}
        type="file"
        accept="application/json,.json"
        hidden
        onChange={(event) => {
          const file = event.target.files?.[0];
          if (file) void readFile(file);
          event.target.value = "";
        }}
      />
    </div>
  );
}
