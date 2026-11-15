import { useEffect, useRef } from "react";

const KEYS: Array<[string, string]> = [
  ["Space", "Play or pause"],
  ["← / →", "Step back or forward"],
  ["J / K", "Previous or next incident"],
  ["+ / −", "Zoom the chart"],
  ["0", "Reset the zoom"],
  ["Esc", "Close the detail panel"],
  ["?", "Show this help"],
];

interface Props {
  open: boolean;
  onClose: () => void;
}

export function ShortcutHelp({ open, onClose }: Props) {
  const closeRef = useRef<HTMLButtonElement | null>(null);

  useEffect(() => {
    if (!open) return;
    closeRef.current?.focus();
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div
      className="modal-backdrop"
      onClick={onClose}
      role="dialog"
      aria-modal="true"
      aria-label="Keyboard shortcuts"
    >
      <div className="modal" onClick={(event) => event.stopPropagation()}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <h3>Keyboard</h3>
          <button
            ref={closeRef}
            className="btn btn-ghost btn-sm"
            type="button"
            onClick={onClose}
            aria-label="Close help"
          >
            ✕
          </button>
        </div>

        <div className="keymap">
          {KEYS.map(([key, description]) => (
            <div key={key} style={{ display: "contents" }}>
              <kbd>{key}</kbd>
              <span className="muted">{description}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
