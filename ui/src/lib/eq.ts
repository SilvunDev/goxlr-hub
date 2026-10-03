// Where the equaliser is drawn: frequencies along a logarithmic axis, gains
// from bottom to top. Nothing here touches the screen.

export const EQ_MIN_HZ = 20;
export const EQ_MAX_HZ = 20000;
/** The equaliser turns a band up or down by this many decibels at most. */
export const EQ_MAX_GAIN = 9;

const DECADES = Math.log10(EQ_MAX_HZ / EQ_MIN_HZ);

function clamp(value: number, min: number, max: number): number {
  return Number.isFinite(value) ? Math.min(max, Math.max(min, value)) : min;
}

export function frequencyToX(hertz: number, width: number): number {
  const bounded = clamp(hertz, EQ_MIN_HZ, EQ_MAX_HZ);
  return (Math.log10(bounded / EQ_MIN_HZ) / DECADES) * width;
}

export function xToFrequency(x: number, width: number): number {
  return EQ_MIN_HZ * 10 ** ((clamp(x, 0, width) / width) * DECADES);
}

export function gainToY(gain: number, height: number): number {
  const bounded = clamp(gain, -EQ_MAX_GAIN, EQ_MAX_GAIN);
  return ((EQ_MAX_GAIN - bounded) / (2 * EQ_MAX_GAIN)) * height;
}

/** The gain drawn at a height, as a whole number of decibels. */
export function yToGain(y: number, height: number): number {
  const gain = EQ_MAX_GAIN - (clamp(y, 0, height) / height) * 2 * EQ_MAX_GAIN;
  return Math.round(gain) || 0;
}

/** A frequency rounded the way it is told: finer where the ear is finer. */
export function snapFrequency(hertz: number): number {
  const step = hertz < 100 ? 0.5 : hertz < 1000 ? 1 : 10;
  return Math.round(hertz / step) * step;
}

/**
 * A frequency moved by one step of the device, a twenty-fourth of an octave,
 * up (`1`) or down (`-1`), and kept within its bounds.
 */
export function nudgeFrequency(hertz: number, direction: 1 | -1, min: number, max: number): number {
  let next = snapFrequency(hertz * 2 ** (direction / 24));
  // Rounding must not swallow the step.
  if (next === hertz) next = hertz + direction * (hertz < 100 ? 0.5 : hertz < 1000 ? 1 : 10);
  return clamp(next, min, max);
}

export interface EqPoint {
  frequency: number;
  gain: number;
}

/**
 * The line through the bands, as points of the drawing: it passes through
 * every band, never swings past what two neighbours ask for, and stays flat
 * beyond the first and the last. It is a picture, not a measurement.
 */
export function curvePoints(
  bands: readonly EqPoint[],
  width: number,
  height: number,
  steps = 96,
): { x: number; y: number }[] {
  // The bands as knots, left to right. Two bands at one place count once.
  const knots: { x: number; y: number }[] = [];
  const placed = bands
    .map((band) => ({ x: frequencyToX(band.frequency, width), y: gainToY(band.gain, height) }))
    .sort((a, b) => a.x - b.x);
  for (const knot of placed) {
    if (knots.at(-1)?.x !== knot.x) knots.push(knot);
  }
  if (knots.length === 0) knots.push({ x: 0, y: gainToY(0, height) });
  if (knots[0].x > 0) knots.unshift({ x: 0, y: knots[0].y });
  const last = knots[knots.length - 1];
  if (last.x < width) knots.push({ x: width, y: last.y });

  // The slope at each knot, chosen so that the line never overshoots
  // (Fritsch-Carlson).
  const rises = knots.slice(1).map((knot, at) => (knot.y - knots[at].y) / (knot.x - knots[at].x));
  const slopes = knots.map((_, at) => {
    const before = rises[at - 1];
    const after = rises[at];
    if (before === undefined || after === undefined || before * after <= 0) return 0;
    return (2 * before * after) / (before + after);
  });

  const xs = new Set(knots.map((knot) => knot.x));
  for (let step = 0; step <= steps; step++) xs.add((step / steps) * width);

  let segment = 0;
  return [...xs]
    .sort((a, b) => a - b)
    .map((x) => {
      while (segment < knots.length - 2 && x > knots[segment + 1].x) segment++;
      const from = knots[segment];
      const to = knots[segment + 1] ?? from;
      const span = to.x - from.x;
      // Two neighbours that agree are joined by a straight line.
      if (x <= from.x || span <= 0 || from.y === to.y) return { x, y: from.y };
      if (x >= to.x) return { x, y: to.y };
      const t = (x - from.x) / span;
      const y =
        (2 * t ** 3 - 3 * t ** 2 + 1) * from.y +
        (t ** 3 - 2 * t ** 2 + t) * span * slopes[segment] +
        (-2 * t ** 3 + 3 * t ** 2) * to.y +
        (t ** 3 - t ** 2) * span * slopes[segment + 1];
      return { x, y: Math.min(height, Math.max(0, y)) };
    });
}
