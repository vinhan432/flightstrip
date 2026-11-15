import { useScrollReveal } from "../hooks/useScrollReveal";

const STEPS = [
  {
    num: "01",
    title: "Record",
    body: "flightstrip watch captures every file change, command and test run as it happens, in order.",
    icon: (
      <>
        <circle cx="12" cy="12" r="8" />
        <circle cx="12" cy="12" r="3" fill="currentColor" />
      </>
    ),
  },
  {
    num: "02",
    title: "Analyze",
    body: "Deterministic detectors flag weakened tests, thrashing, leaked secrets and dangerous commands.",
    icon: (
      <>
        <path d="M3 17l5-5 4 3 5-7 4 4" />
        <path d="M3 21h18" />
      </>
    ),
  },
  {
    num: "03",
    title: "Review",
    body: "Share the replay and the trust report with your team, before the pull request is merged.",
    icon: (
      <>
        <path d="M4 4h16v12H4z" />
        <path d="M8 20h8" />
        <path d="M12 16v4" />
      </>
    ),
  },
];

export function HowItWorks() {
  const [ref, visible] = useScrollReveal<HTMLDivElement>();

  return (
    <div className="briefing" ref={ref}>
      {STEPS.map((step, i) => (
        <article
          key={step.num}
          className={`card briefing-card reveal${visible ? " is-visible" : ""}`}
          style={{ animationDelay: `${i * 100}ms` }}
        >
          <div className="num">{step.num}</div>
          <h3>{step.title}</h3>
          <p>{step.body}</p>
          <svg
            className="icon"
            width="40"
            height="40"
            viewBox="0 0 24 24"
            fill="none"
            stroke="var(--ink-light)"
            strokeWidth="1.4"
            strokeLinecap="round"
            strokeLinejoin="round"
            aria-hidden="true"
          >
            {step.icon}
          </svg>
        </article>
      ))}
    </div>
  );
}
