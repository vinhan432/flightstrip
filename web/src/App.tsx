// Placeholder shell for Task 1/3. Replaced in Task 12 with the assembled page.
import { StoreProvider, useStore } from "./store";

function Scaffold() {
  const { session, status, error, manifest } = useStore();
  return (
    <main className="page">
      <h1>Flightstrip</h1>
      <p className="lede">
        {status === "ready" && session
          ? `${session.meta.title} — ${session.steps.length} steps, ${session.incidents.length} incidents`
          : status === "error"
            ? `Error: ${error}`
            : "Loading…"}
      </p>
      <p className="stamp">{manifest.length} sessions in the catalogue</p>
    </main>
  );
}

export function App() {
  return (
    <StoreProvider>
      <Scaffold />
    </StoreProvider>
  );
}
