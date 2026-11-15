import type { SessionManifestEntry } from "../store";

interface Props {
  manifest: SessionManifestEntry[];
  activeId: string | null;
  onPick: (id: string) => void;
}

/** The catalogue: three real recordings, each with its hook and severity. */
export function SessionPicker({ manifest, activeId, onPick }: Props) {
  if (manifest.length === 0) {
    return (
      <p className="muted" style={{ fontSize: "0.8rem" }}>
        Loading the catalogue…
      </p>
    );
  }

  return (
    <div className="picker" role="group" aria-label="Recorded sessions">
      {manifest.map((entry) => (
        <button
          key={entry.id}
          type="button"
          aria-pressed={entry.id === activeId}
          onClick={() => onPick(entry.id)}
        >
          <span className={`sev ${entry.severity === "info" ? "sev-info" : entry.severity === "warn" ? "sev-warn" : "sev-critical"}`} aria-hidden="true" />
          <strong>{entry.title}</strong>
          <br />
          <span className="muted" style={{ fontSize: "0.74rem" }}>
            {entry.hook}
          </span>
        </button>
      ))}
    </div>
  );
}
