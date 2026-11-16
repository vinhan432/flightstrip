// The incident track: every marker is present, and each stamps in as the pen
// reaches it. The "present but not yet stamped" distinction is the whole
// point — a finished recording must not show an empty incident track.

import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";
import { IncidentTrack } from "./IncidentTrack";
import type { Session } from "../../types";
import risky from "../../../../fixtures/risky-session.json";

const session = risky as unknown as Session;
const view = { t0: 0, t1: session.meta.duration_ms, width: 1200 };

function renderAt(clockMs: number) {
  return render(
    <svg>
      <IncidentTrack
        session={session}
        view={view}
        height={48}
        clockMs={clockMs}
        onSelectIncident={() => {}}
      />
    </svg>,
  );
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
    const stamped = container.querySelectorAll(".mark-incident.is-stamped");
    expect(stamped.length).toBe(1);
  });

  it("stamps every incident once the pen is past them all", () => {
    const { container } = renderAt(session.meta.duration_ms);
    const stamped = container.querySelectorAll(".mark-incident.is-stamped");
    expect(stamped.length).toBe(session.incidents.length);
  });

  it("carries severity in a label, not only in colour", () => {
    const { container } = renderAt(session.meta.duration_ms);
    const marker = container.querySelector(".mark-incident");
    expect(marker?.getAttribute("aria-label")).toMatch(/critical|warn|info/);
  });
});
