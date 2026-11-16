import type { Session } from "../types";
import { useClock } from "../hooks/useClock";
import type { Playback } from "../hooks/usePlayback";
import { formatClock } from "../utils/format";

interface Props {
  session: Session | null;
  playback: Playback;
  steps: number;
}

/** The instrument badge in the hero corner: version, REC, live counters. */
export function DataPlate({ session, playback, steps }: Props) {
  // Only this badge re-renders as the clock ticks.
  const clockMs = useClock(playback);

  return (
    <div className="data-plate plate" role="status" aria-label="Recorder status">
      <div className="row">
        <span className="stamp">Flightstrip</span>
        <span className="data">v0.1.0</span>
      </div>
      <div className="row">
        <span className="rec">
          <span className="dot blink" aria-hidden="true" />
          <span className="stamp">REC</span>
        </span>
        <span className="data">{formatClock(clockMs)}</span>
      </div>
      <div className="row">
        <span className="stamp">Steps</span>
        <span className="data">{steps}</span>
      </div>
      {session && (
        <div className="row">
          <span className="stamp">Repo</span>
          <span className="data">{session.meta.repo_name}</span>
        </div>
      )}
    </div>
  );
}
