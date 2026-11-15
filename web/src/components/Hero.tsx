import { ParticleField } from "./ParticleField";

const HEADLINE = "See what your AI agent actually did.";
const STAGGER_MS = 40;

interface Props {
  onWatch: () => void;
  onDownload: () => void;
}

/**
 * The first load. Full viewport, the heading revealed letter by letter.
 *
 * The letters are individual spans for the animation, so the full sentence
 * is carried on the h1's aria-label: a screen reader reads the sentence,
 * not the alphabet.
 */
export function Hero({ onWatch, onDownload }: Props) {
  let index = 0;

  return (
    <section className="hero" id="top">
      <ParticleField />
      <div className="vignette" aria-hidden="true" />

      <div className="hero-inner">
        <h1 aria-label={HEADLINE}>
          {HEADLINE.split("").map((char, i) => {
            if (char === " ") {
              return <span key={i}> </span>;
            }
            const delay = index++ * STAGGER_MS;
            return (
              <span
                key={i}
                className="letter"
                aria-hidden="true"
                style={{ animationDelay: `${delay}ms` }}
              >
                {char}
              </span>
            );
          })}
        </h1>

        <p className="lede">
          Flightstrip records every file change, command, and test run. Then it
          shows you the truth.
        </p>

        <div className="hero-actions">
          <button className="btn btn-primary" type="button" onClick={onWatch}>
            Watch a Replay
          </button>
          <button className="btn btn-secondary" type="button" onClick={onDownload}>
            Download CLI
          </button>
        </div>
      </div>
    </section>
  );
}
