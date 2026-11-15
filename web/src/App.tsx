// The page, top to bottom. Layout only: all state lives in useSession.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Hero } from "./components/Hero";
import { DataPlate } from "./components/DataPlate";
import { StripChart } from "./components/StripChart/StripChart";
import { Scrubber } from "./components/StripChart/Scrubber";
import { CVRPanel } from "./components/CVRPanel";
import { IncidentReport } from "./components/IncidentReport";
import { TraceGraph } from "./components/TraceGraph";
import { HowItWorks } from "./components/HowItWorks";
import { DownloadButton } from "./components/DownloadButton";
import { DropZone } from "./components/DropZone";
import { SessionPicker } from "./components/SessionPicker";
import { ShortcutHelp } from "./components/ShortcutHelp";
import { Footer } from "./components/Footer";
import { StoreProvider, useStore } from "./store";
import { useSession } from "./hooks/useSession";
import { useKeyboard } from "./hooks/useKeyboard";
import { formatClock } from "./utils/format";
import type { Incident } from "./types";

function Page() {
  const {
    manifest,
    session,
    status,
    error,
    loadSession,
    loadFromFile,
    loadFromText,
    toast,
    showToast,
  } = useStore();

  const {
    playback,
    view,
    setView,
    focus,
    openStep,
    closeFocus,
    route,
    navigate,
  } = useSession();

  const chartRef = useRef<HTMLDivElement | null>(null);
  const [helpOpen, setHelpOpen] = useState(false);

  // The chart's rendered width drives the view's coordinate space.
  useEffect(() => {
    const el = chartRef.current;
    if (!el) return;
    const observer = new ResizeObserver(([entry]) => {
      const next = Math.max(320, Math.floor(entry.contentRect.width));
      setView((current) =>
        current.width === next ? current : { ...current, width: next },
      );
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, [session, setView]);

  // Autoplay once, when the chart first scrolls into view.
  const played = useRef(false);
  useEffect(() => {
    const el = chartRef.current;
    if (!el || !session) return;
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting && !played.current) {
          played.current = true;
          playback.play();
        }
      },
      { threshold: 0.3 },
    );
    observer.observe(el);
    return () => observer.disconnect();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [session]);

  const nextIncident = useCallback(
    (direction: 1 | -1) => {
      if (!session) return;
      const incidents = [...session.incidents].sort((a, b) => a.step - b.step);
      const current = focus?.incident?.id;
      const index = incidents.findIndex((i) => i.id === current);
      const next =
        index === -1
          ? direction === 1
            ? incidents[0]
            : incidents[incidents.length - 1]
          : incidents[index + direction];
      if (next) openStep(next.step, next);
    },
    [session, focus, openStep],
  );

  const keys = useMemo(
    () => ({
      " ": () => playback.toggle(),
      ArrowRight: () => playback.step(1),
      ArrowLeft: () => playback.step(-1),
      j: () => nextIncident(1),
      k: () => nextIncident(-1),
      "+": () => setView((v) => zoom(v, 0.8)),
      "=": () => setView((v) => zoom(v, 0.8)),
      "-": () => setView((v) => zoom(v, 1.25)),
      _: () => setView((v) => zoom(v, 1.25)),
      "0": () => setView((v) => ({ ...v, t0: 0, t1: v.t1 - v.t0 + v.t0 })),
      "?": () => setHelpOpen((open) => !open),
      Escape: () => {
        if (helpOpen) setHelpOpen(false);
        else closeFocus();
      },
    }),
    [playback, nextIncident, closeFocus, helpOpen, setView],
  );
  useKeyboard(keys);

  const onSelectIncident = useCallback(
    (incident: Incident) => {
      openStep(incident.step, incident);
      navigate({ sessionId: route.sessionId, focus: incident.id });
    },
    [openStep, navigate, route.sessionId],
  );

  return (
    <>
      <a className="visually-hidden" href="#chart">
        Skip to the replay
      </a>

      <div style={{ position: "relative" }}>
        <DataPlate
          session={session}
          clockMs={playback.clockMs}
          steps={session?.steps.length ?? 0}
        />
        <Hero
          onWatch={() =>
            document.getElementById("chart")?.scrollIntoView({ behavior: "smooth" })
          }
          onDownload={() =>
            document.getElementById("download")?.scrollIntoView({ behavior: "smooth" })
          }
        />
      </div>

      <main className="page">
        {/* ── the recorder ─────────────────────────────────────────── */}
        <section className="section" id="chart">
          <div className="section-head">
            <span className="stamp">Flight Data</span>
            <span className="rule is-drawn" />
          </div>

          <div style={{ marginBottom: "16px" }}>
            <SessionPicker
              manifest={manifest}
              activeId={route.sessionId}
              onPick={(id) => {
                played.current = false;
                void loadSession(id);
                navigate({ sessionId: id });
              }}
            />
          </div>

          {status === "error" && (
            <p role="alert" className="lede" style={{ color: "var(--red)" }}>
              Could not load the session: {error}
            </p>
          )}

          {status === "loading" && (
            <div className="skeleton" style={{ height: 308 }} aria-hidden="true" />
          )}

          {session && status !== "loading" && (
            <div className="chart-layout">
              <div>
                <div className="transport">
                  <button
                    className="btn btn-ghost btn-sm"
                    type="button"
                    onClick={playback.toggle}
                    aria-label={playback.playing ? "Pause the replay" : "Play the replay"}
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
                  <span className="clock data">
                    {formatClock(playback.clockMs)}
                    <span className="faint"> / {formatClock(view.t1)}</span>
                  </span>
                </div>

                <div ref={chartRef}>
                  <StripChart
                    session={session}
                    view={view}
                    clockMs={playback.clockMs}
                    onViewChange={setView}
                    onSelectStep={(index) => openStep(index)}
                    onSelectIncident={(id) => {
                      const incident = session.incidents.find((i) => i.id === id);
                      if (incident) onSelectIncident(incident);
                    }}
                  />
                </div>

                <Scrubber
                  session={session}
                  clockMs={playback.clockMs}
                  onSeek={playback.seek}
                  onSelectIncident={(id) => {
                    const incident = session.incidents.find((i) => i.id === id);
                    if (incident) onSelectIncident(incident);
                  }}
                />

                <p className="stamp" style={{ marginTop: "16px" }}>
                  Scroll to zoom · drag to pan · click any mark · press ? for keys
                </p>
              </div>

              <div>
                {focus ? (
                  <CVRPanel
                    session={session}
                    stepIndex={focus.stepIndex}
                    incident={focus.incident}
                    onClose={closeFocus}
                    onJump={(index) => openStep(index)}
                  />
                ) : (
                  <aside className="cvr" aria-label="Step detail">
                    <p className="stamp">Cockpit Voice Recorder</p>
                    <p className="muted" style={{ fontSize: "0.82rem", marginTop: "12px" }}>
                      Click a step on the tape, a prompt marker, or an incident
                      triangle to read the diff and the prompt that caused it.
                    </p>
                  </aside>
                )}
              </div>
            </div>
          )}
        </section>

        {/* ── the investigation ────────────────────────────────────── */}
        {session && (
          <section className="section" id="report">
            <div className="section-head">
              <span className="stamp">The Investigation</span>
              <span className="rule is-drawn" />
            </div>
            <IncidentReport session={session} onSelectIncident={onSelectIncident} />
          </section>
        )}

        {/* ── the network ──────────────────────────────────────────── */}
        {session && session.prompts.length > 0 && (
          <section className="section" id="trace">
            <div className="section-head">
              <span className="stamp">Trace Map</span>
              <span className="rule is-drawn" />
            </div>
            <div className="desktop-only">
              <TraceGraph
                session={session}
                onJumpToStep={(step) => {
                  openStep(step);
                  document.getElementById("chart")?.scrollIntoView({ behavior: "smooth" });
                }}
              />
            </div>
            <p className="mobile-only muted" style={{ fontSize: "0.82rem" }}>
              The trace map is available on a wider screen.
            </p>
          </section>
        )}

        {/* ── the briefing ─────────────────────────────────────────── */}
        <section className="section" id="how">
          <div className="section-head">
            <span className="stamp">The Briefing</span>
            <span className="rule is-drawn" />
          </div>
          <HowItWorks />
        </section>

        {/* ── the supply drop ──────────────────────────────────────── */}
        <section className="section" id="download">
          <div className="section-head">
            <span className="stamp">Get Flightstrip</span>
            <span className="rule is-drawn" />
          </div>
          <DownloadButton />
        </section>

        {/* ── load your own ────────────────────────────────────────── */}
        <section className="section" id="load">
          <div className="section-head">
            <span className="stamp">Load Your Own</span>
            <span className="rule is-drawn" />
          </div>
          <DropZone
            onLoad={(loaded) => {
              played.current = false;
              loadFromText(JSON.stringify(loaded));
              document.getElementById("chart")?.scrollIntoView({ behavior: "smooth" });
            }}
            onError={(message) => showToast(message)}
          />
        </section>
      </main>

      <Footer />
      <ShortcutHelp open={helpOpen} onClose={() => setHelpOpen(false)} />

      {toast && (
        <div className="toast" role="status" aria-live="polite">
          {toast}
        </div>
      )}

      {/* Screen readers hear the current step as it changes. */}
      <div className="visually-hidden" aria-live="polite">
        {session
          ? `Step ${focus?.stepIndex ?? 0} of ${session.steps.length} at ${formatClock(playback.clockMs)}`
          : ""}
      </div>

      <input
        type="file"
        hidden
        aria-hidden="true"
        onChange={(event) => {
          const file = event.target.files?.[0];
          if (file) void loadFromFile(file);
        }}
      />
    </>
  );
}

/** Zoom the view around its centre. */
function zoom(view: { t0: number; t1: number; width: number }, factor: number) {
  const centre = (view.t0 + view.t1) / 2;
  const span = Math.max(500, (view.t1 - view.t0) * factor);
  return { ...view, t0: centre - span / 2, t1: centre + span / 2 };
}

export function App() {
  return (
    <StoreProvider>
      <Page />
    </StoreProvider>
  );
}
