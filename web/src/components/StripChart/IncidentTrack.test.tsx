// The incident track: every marker is present, and each stamps in as the pen
// reaches it. The "present but not yet stamped" distinction is the whole
// point — a finished recording must not show an empty incident track.
//
// The set of reached incidents is the only thing that changes during
// playback, so the component must not re-render on every tick.

import { describe, it, expect } from "vitest";
import { render, act } from "@testing-library/react";
import { IncidentTrack } from "./IncidentTrack";
import { makePlayback } from "../../test/fakePlayback";
import type { Session } from "../../types";
import risky from "../../../../fixtures/risky-session.json";

const session = risky as unknown as Session;
const view = { t0: 0, t1: session.meta.duration_ms, width: 1200 };

function renderAt(clockMs: number) {
  const playback = makePlayback(clockMs);
  const utils = render(
    <svg>
      <IncidentTrack
        session={session}
        view={view}
        height={48}
        playback={playback}
        onSelectIncident={() => {}}
      />
    </svg>,
  );
  return { ...utils, playback };
}

describe("IncidentTrack", () => {
  it("renders one marker per incident", () => {
    const { container } = renderAt(0);
    expect(container.querySelectorAll(".mark-incident").length).toBe(
      session.incidents.length,
    );
  });

  it("marks only the incidents the pen has reached", () => {
    // At 13s only the first incident (step 3) has been passed.
    const { container } = renderAt(13000);
    expect(container.querySelectorAll(".mark-incident.is-stamped").length).toBe(
      1,
    );
  });

  it("stamps every incident once the pen is past them all", () => {
    const { container } = renderAt(session.meta.duration_ms);
    expect(container.querySelectorAll(".mark-incident.is-stamped").length).toBe(
      session.incidents.length,
    );
  });

  it("stamps a marker as the clock advances", () => {
    const { container, playback } = renderAt(0);
    expect(container.querySelectorAll(".mark-incident.is-stamped").length).toBe(
      0,
    );

    act(() => playback.setClock(13000));
    expect(container.querySelectorAll(".mark-incident.is-stamped").length).toBe(
      1,
    );
  });

  it("does not re-render when the clock moves without crossing an incident", () => {
    const { container, playback } = renderAt(0);

    // 20 ticks that cross no incident boundary.
    act(() => {
      for (let ms = 0; ms < 13000; ms += 600) playback.setClock(ms);
    });

    // Still nothing stamped, and the DOM is unchanged.
    expect(container.querySelectorAll(".mark-incident.is-stamped").length).toBe(
      0,
    );
  });

  it("carries severity in a label, not only in colour", () => {
    const { container } = renderAt(session.meta.duration_ms);
    const marker = container.querySelector(".mark-incident");
    expect(marker?.getAttribute("aria-label")).toMatch(/critical|warn|info/);
  });
});
