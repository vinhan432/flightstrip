// The supply drop.
//
// There is no published crate and no GitHub release, so this does not link
// to a file that does not exist. It names the real install path — a release
// build from source — and copies the command. The OS detection is still
// here, because it is what tells a person which binary they will get.

import { useCallback, useMemo, useState } from "react";

type Os = "windows" | "linux" | "macos" | "unknown";

function detectOs(): Os {
  if (typeof navigator === "undefined") return "unknown";
  const ua = navigator.userAgent;
  if (/Win/i.test(ua)) return "windows";
  if (/Mac/i.test(ua)) return "macos";
  if (/Linux|X11/i.test(ua)) return "linux";
  return "unknown";
}

const BINARY: Record<Os, string> = {
  windows: "flightstrip-x86_64-pc-windows-msvc.exe",
  linux: "flightstrip-x86_64-unknown-linux-gnu",
  macos: "flightstrip-aarch64-apple-darwin",
  unknown: "flightstrip",
};

const LABEL: Record<Os, string> = {
  windows: "Windows",
  linux: "Linux",
  macos: "macOS",
  unknown: "your platform",
};

const BUILD_CMD = "cargo build --release";

export function DownloadButton() {
  const os = useMemo(detectOs, []);
  const [copied, setCopied] = useState(false);
  const [ripples, setRipples] = useState<Array<{ id: number; x: number; y: number }>>([]);
  const [pressed, setPressed] = useState(false);

  const copy = useCallback(async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 2000);
    } catch {
      setCopied(false);
    }
  }, []);

  const onMove = useCallback((event: React.MouseEvent<HTMLButtonElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    setRipples((current) => [
      ...current.slice(-3),
      { id: Date.now(), x: event.clientX - rect.left, y: event.clientY - rect.top },
    ]);
  }, []);

  return (
    <div>
      <button
        className={`download${pressed ? " is-pressed" : ""}`}
        type="button"
        onMouseMove={onMove}
        onMouseDown={() => setPressed(true)}
        onMouseUp={() => setPressed(false)}
        onMouseLeave={() => setPressed(false)}
        onClick={() => void copy(BUILD_CMD)}
        aria-label={`Copy the build command for ${LABEL[os]}`}
      >
        {ripples.map((r) => (
          <span
            key={r.id}
            className="ripple"
            style={{ left: r.x, top: r.y }}
            aria-hidden="true"
          />
        ))}

        {copied ? (
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <path d="M20 6 9 17l-5-5" />
          </svg>
        ) : (
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <path d="M12 3v12" />
            <path d="m7 10 5 5 5-5" />
            <path d="M5 21h14" />
          </svg>
        )}

        <span>{copied ? "Command copied" : `Build for ${LABEL[os]}`}</span>
      </button>

      <div className="install-line">
        <code className="mono">
          cargo build --release
        </code>
        <span className="muted" style={{ fontSize: "0.75rem" }}>
          → target/release/{BINARY[os]}
        </span>
        <button
          className="copy-btn"
          type="button"
          onClick={() => void copy(BUILD_CMD)}
          aria-label="Copy the build command"
        >
          {copied ? "copied" : "copy"}
        </button>
      </div>

      <p className="muted" style={{ fontSize: "0.78rem", marginTop: "12px" }}>
        Windows is the first target; the macOS and Linux builds are untested.
        Rust stable, no other toolchain, no C compiler.
      </p>
    </div>
  );
}
