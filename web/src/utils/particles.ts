// The particle maths. Pure functions so the drift, the wrap and the
// repulsion can be tested without a canvas.
//
// `step` takes a delta in milliseconds and scales to a 16ms frame, so the
// drift is frame-rate independent and a 30fps loop looks the same as 60fps.

export interface Particle {
  x: number;
  y: number;
  vx: number;
  vy: number;
  radius: number;
  opacity: number;
}

/** Drift speeds, in pixels per 16ms frame, per the spec. */
const MIN_SPEED = 0.1;
const MAX_SPEED = 0.3;

function randomBetween(min: number, max: number): number {
  return min + Math.random() * (max - min);
}

export function makeParticles(
  count: number,
  width: number,
  height: number,
): Particle[] {
  const particles: Particle[] = [];
  for (let i = 0; i < count; i++) {
    // Sign is random so the field drifts in both directions, but the
    // magnitude stays inside the spec's band.
    const vx = randomBetween(MIN_SPEED, MAX_SPEED);
    const vy = randomBetween(-MAX_SPEED, -MIN_SPEED);
    particles.push({
      x: Math.random() * width,
      y: Math.random() * height,
      vx,
      vy,
      radius: randomBetween(1, 2),
      opacity: randomBetween(0.25, 0.7),
    });
  }
  return particles;
}

/** Advance one particle by `dt` milliseconds, wrapping at the edges. */
export function step(
  p: Particle,
  width: number,
  height: number,
  dt: number,
): void {
  const frames = dt / 16;
  p.x += p.vx * frames;
  p.y += p.vy * frames;

  if (p.x < 0) p.x += width;
  else if (p.x >= width) p.x -= width;

  if (p.y < 0) p.y += height;
  else if (p.y >= height) p.y -= height;
}

/**
 * A gentle push away from the cursor. Uses squared distance so the common
 * case (far away) costs no square root.
 */
export function repel(
  p: Particle,
  mx: number,
  my: number,
  radius: number,
): void {
  const dx = p.x - mx;
  const dy = p.y - my;
  const distSq = dx * dx + dy * dy;
  const radiusSq = radius * radius;

  if (distSq > radiusSq || distSq === 0) return;

  const dist = Math.sqrt(distSq);
  // Stronger the closer it is, and never zero, so a cursor sitting exactly
  // on a particle still resolves.
  const force = (1 - dist / radius) * 0.6;
  p.vx += (dx / dist) * force;
  p.vy += (dy / dist) * force;
}

/** Index pairs closer than `maxDist`. O(n²), which is fine at n≈100. */
export function connections(
  particles: Particle[],
  maxDist: number,
): Array<[number, number]> {
  const pairs: Array<[number, number]> = [];
  const maxSq = maxDist * maxDist;

  for (let i = 0; i < particles.length; i++) {
    for (let j = i + 1; j < particles.length; j++) {
      const dx = particles[i].x - particles[j].x;
      const dy = particles[i].y - particles[j].y;
      if (dx * dx + dy * dy < maxSq) pairs.push([i, j]);
    }
  }
  return pairs;
}
