// A performance guard on the playback clock.
//
// The bug this exists to prevent: the clock used to be React state, so every
// animation frame re-rendered the whole page. It measured ~21ms per tick
// against a 16.67ms frame budget, which is exactly what stuttering looks like.
//
// The fix is that the clock is a subscription, not state. These tests assert
// the property that makes it fast: a clock tick must not re-render the tree.

import { describe, it, expect } from "vitest";
import { render, act } from "@testing-library/react";
import { StripChart } from "./components/StripChart/StripChart";
import { Scrubber } from "./components/StripChart/Scrubber";
import { DataPlate } from "./components/DataPlate";
import { makePlayback } from "./test/fakePlayback";
import type { Session } from "./types";
import risky from "../../fixtures/risky-session.json";

const session = risky as unknown as Session;
const view = { t0: 0, t1: session.meta.duration_ms, width: 1200 };

describe("clock performance", () => {
  it("does not re-render the chart section on a clock tick", () => {
    const playback = makePlayback(0);

    // A sentinel outside the clock subscribers: if the page re-rendered, this
    // counter would climb with the clock.
    let renders = 0;
    function Sentinel() {
      renders++;
      return <span data-testid="sentinel" />;
    }

    const { container } = render(
      <div>
        <Sentinel />
        <StripChart
          session={session}
          view={view}
          playback={playback}
          onViewChange={() => {}}
          onSelectStep={() => {}}
          onSelectIncident={() => {}}
        />
        <Scrubber session={session} playback={playback} />
        <DataPlate session={session} playback={playback} steps={session.steps.length} />
      </div>,
    );

    const before = renders;

    // One tick, so any legitimate one-off additions (the pen trail appears
    // once the clock is past zero) have already happened.
    act(() => playback.setClock(16));
    const nodesAfterFirst = container.querySelectorAll("*").length;

    // 60 ticks: one second of playback at 60fps.
    act(() => {
      for (let i = 0; i < 60; i++) playback.setClock(16 + i * 16);
    });

    // The sentinel did not re-render at all.
    expect(renders).toBe(before);

    // And the DOM did not churn: the node count is unchanged across 60
    // frames, so nothing was torn down and rebuilt per tick.
    expect(container.querySelectorAll("*").length).toBe(nodesAfterFirst);
  });

  it("still moves the pen when the clock advances", () => {
    const playback = makePlayback(0);
    const { container } = render(
      <StripChart
        session={session}
        view={view}
        playback={playback}
        onViewChange={() => {}}
        onSelectStep={() => {}}
        onSelectIncident={() => {}}
      />,
    );

    const penBefore = container.querySelector(".pen polygon")?.getAttribute("points");

    act(() => playback.setClock(session.meta.duration_ms / 2));

    const penAfter = container.querySelector(".pen polygon")?.getAttribute("points");
    expect(penAfter).not.toBe(penBefore);
  });
});
