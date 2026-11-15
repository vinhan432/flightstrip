export function Footer() {
  return (
    <footer className="footer page">
      <p style={{ margin: 0 }}>
        Detectors are deterministic rules over the recording — no model, no
        judgement of correctness.
      </p>

      <div style={{ display: "flex", gap: "12px", alignItems: "center" }}>
        <span className="badge">Built with Freebuff</span>
        <span className="badge">Hackathon 2026</span>
        {/* A GitHub link goes here once a remote exists. A dead link on a
            page judged on polish is worse than no link. */}
      </div>
    </footer>
  );
}
