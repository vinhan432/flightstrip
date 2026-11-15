// Largest-Triangle-Three-Buckets. Reduces a series to at most `maxPoints`
// while keeping the shape a person would recognise: peaks and troughs
// survive, flat runs collapse. Pure, no DOM.

export interface Point {
  t: number;
  churn: number;
}

export function downsample(points: Point[], maxPoints: number): Point[] {
  if (maxPoints <= 0) return [];
  if (points.length <= maxPoints) return points;
  if (maxPoints < 3) return [points[0], points[points.length - 1]];

  const sampled: Point[] = [];
  const every = (points.length - 2) / (maxPoints - 2);

  // The ends are always kept: they anchor the trace.
  sampled.push(points[0]);

  let a = 0;
  for (let i = 0; i < maxPoints - 2; i++) {
    // The average of the three points that bound this bucket.
    const avgRangeStart = Math.floor((i + 1) * every) + 1;
    const avgRangeEnd = Math.floor((i + 2) * every) + 1;
    const avgRangeLength = Math.min(avgRangeEnd, points.length) - avgRangeStart;

    let avgT = 0;
    let avgChurn = 0;
    for (let j = avgRangeStart; j < avgRangeStart + avgRangeLength; j++) {
      avgT += points[j].t;
      avgChurn += points[j].churn;
    }
    if (avgRangeLength > 0) {
      avgT /= avgRangeLength;
      avgChurn /= avgRangeLength;
    }

    const rangeOffs = Math.floor(i * every) + 1;
    const rangeTo = Math.floor((i + 1) * every) + 1;

    // Pick the point in this bucket furthest from the line a→average.
    const pointA = points[a];
    let maxArea = -1;
    let chosen = rangeOffs;

    for (let j = rangeOffs; j < rangeTo && j < points.length; j++) {
      const area = Math.abs(
        (pointA.t - avgT) * (points[j].churn - pointA.churn) -
          (pointA.t - points[j].t) * (avgChurn - pointA.churn),
      );
      if (area > maxArea) {
        maxArea = area;
        chosen = j;
      }
    }

    sampled.push(points[chosen]);
    a = chosen;
  }

  sampled.push(points[points.length - 1]);
  return sampled;
}
