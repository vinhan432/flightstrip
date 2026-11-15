// Intermediate shell. Task 12 assembles the full page.
import { useEffect, useMemo, useRef, useState } from "react";
import { Hero } from "./components/Hero";
import { DataPlate } from "./components/DataPlate";
import { StripChart } from "./components/StripChart/StripChart";
import { Scrubber } from "./components/StripChart/Scrubber";
import { StoreProvider, useStore } from "./store";
import { usePlayback } from "./hooks/usePlayback";
import { useKeyboard } from "./hooks/useKeyboard";
import { useHashRoute } from "./hooks/useHashRoute";
import { domain } from "./utils/layout";

function Shell() {
  const { session, status, error } = useStore();
  // Routing is wired into the selection in Task 12; parsed here so the hook
  // is exercised and the deep-link path is live from the start.
  const [_route, _navigate] = useHashRoute();
  const chartRef = useRef<HTMLDivElement | null>(null);
  const [width, setWidth] = useState(1200);

  const duration = session ? domain(session).end : 0;
  const playback = usePlayback(session, duration);

  const [view, setView] = useState({ t0: 0, t1: 1, width: 1200 });

  // Reset the view when the session changes.
  useEffect(() => {
    if (!session) return;
    setView({ t0: 0, t1: Math.max(1, duration), width });
  }, [session, duration, width]);

  // Track the chart's rendered width.
  useEffect(() => {
    const el = chartRef.current;
    if (!el) return;
    const observer = new ResizeObserver(([entry]) => {
      setWidth(Math.max(320, Math.floor(entry.contentRect.width)));
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, [session]);

  // Autoplay once, when the chart first scrolls into view.
  const playedRef = useRef(false);
  useEffect(() => {
    const el = chartRef.current;
    if (!el || !session) return;
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting && !playedRef.current) {
          playedRef.current = true;
          playback.play();
        }
      },
      { threshold: 0.3 },
    );
    observer.observe(el);
    return () => observer.disconnect();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [session]);

  const keys = useMemo(
    () => ({
      " ": () => playback.toggle(),
      ArrowRight: () => playback.step(1),
      ArrowLeft: () => playback.step(-1),
      k: () => playback.step(-1),
      j: () => playback.step(1),
    }),
    [playback],
  );
  useKeyboard(keys);

  return (
    <>
      <DataPlate
        session={session}
        clockMs={playback.clockMs}
        steps={session?.steps.length ?? 0}
      />
      <Hero
        onWatch={() =>
          chartRef.current?.scrollIntoView({ behavior: "smooth" })
        }
        onDownload={() =>
          document.getElementById("download")?.scrollIntoView({ behavior: "smooth" })
        }
      />

      <main className="page">
        <section className="section" id="chart">
          <div className="section-head">
            <span className="stamp">Flight Data</span>
            <span className="rule is-drawn" />
          </div>

          {status === "error" && (
            <p className="lede" role="alert">
              Could not load the session: {error}
            </p>
          )}

          {status === "loading" && (
            <div className="skeleton" style={{ height: 308 }} />
          )}

          {session && status === "ready" && (
            <>
              <div className="transport">
                <button
                  className="btn btn-ghost btn-sm"
                  type="button"
                  onClick={playback.toggle}
                  aria-label={playback.playing ? "Pause" : "Play"}
                >
                  {playback.playing ? "❚❚ Pause" : "▶ Play"}
                </button>
                <button
                  className="btn btn-ghost btn-sm"
                  type="button"
                  onClick={playback.reset}
                >
                  ↺ Start
                </button>
                <div className="speeds" role="group" aria-label="Playback speed">
                  {[1, 2, 4].map((s) => (
                    <button
                      key={s}
                      type="button"
                      aria-pressed={playback.speed === s}
                      onClick={() => playback.setSpeed(s)}
                    >
                      {s}×
                    </button>
                  ))}
                </div>
              </div>

              <div ref={chartRef}>
                <StripChart
                  session={session}
                  view={view}
                  clockMs={playback.clockMs}
                  onViewChange={setView}
                  onSelectStep={() => {}}
                  onSelectIncident={() => {}}
                />
              </div>

              <Scrubber
                session={session}
                clockMs={playback.clockMs}
                onSeek={playback.seek}
              />
            </>
          )}
        </section>
      </main>
    </>
  );
}

export function App() {
  return (
    <StoreProvider>
      <Shell />
    </StoreProvider>
  );
}
