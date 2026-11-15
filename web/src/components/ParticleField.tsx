import { useParticleCanvas } from "../hooks/useParticleCanvas";

/** The cockpit dust: a full-bleed canvas behind the hero content. */
export function ParticleField() {
  const ref = useParticleCanvas();
  return (
    <canvas
      ref={ref}
      className="particle-field"
      aria-hidden="true"
      style={{ position: "absolute", inset: 0, zIndex: 0, width: "100%", height: "100%" }}
    />
  );
}
