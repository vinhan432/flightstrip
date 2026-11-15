// Intermediate shell. Task 12 assembles the full page.
import { Hero } from "./components/Hero";
import { DataPlate } from "./components/DataPlate";
import { StoreProvider, useStore } from "./store";

function Shell() {
  const { session, status, error, manifest } = useStore();
  return (
    <>
      <DataPlate
        session={session}
        clockMs={0}
        steps={session?.steps.length ?? 0}
      />
      <Hero
        onWatch={() => {
          document.getElementById("chart")?.scrollIntoView({ behavior: "smooth" });
        }}
        onDownload={() => {
          document.getElementById("download")?.scrollIntoView({ behavior: "smooth" });
        }}
      />
      <main className="page">
        <p className="lede">
          {status === "ready" && session
            ? `${session.meta.title} — ${session.steps.length} steps, ${session.incidents.length} incidents`
            : status === "error"
              ? `Error: ${error}`
              : "Loading…"}
        </p>
        <p className="stamp">{manifest.length} sessions in the catalogue</p>
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
