import { describe, expect, it } from 'vitest';
import {
  curvePoints,
  frequencyToX,
  gainToY,
  nudgeFrequency,
  snapFrequency,
  xToFrequency,
  yToGain,
} from './eq';

describe('the equaliser drawing', () => {
  it('spreads the frequencies evenly by decade', () => {
    expect(frequencyToX(20, 900)).toBe(0);
    expect(frequencyToX(200, 900)).toBeCloseTo(300);
    expect(frequencyToX(2000, 900)).toBeCloseTo(600);
    expect(frequencyToX(20000, 900)).toBeCloseTo(900);
  });

  it('reads a frequency back from where it is drawn', () => {
    for (const hertz of [31.5, 250, 1000, 16000]) {
      expect(xToFrequency(frequencyToX(hertz, 720), 720)).toBeCloseTo(hertz, 3);
    }
  });

  it('never leaves the drawing', () => {
    expect(frequencyToX(1, 900)).toBe(0);
    expect(frequencyToX(1e9, 900)).toBe(900);
    expect(frequencyToX(Number.NaN, 900)).toBe(0);
    expect(xToFrequency(-50, 900)).toBe(20);
    expect(xToFrequency(5000, 900)).toBeCloseTo(20000);
    expect(gainToY(40, 180)).toBe(0);
    expect(gainToY(-40, 180)).toBe(180);
    expect(yToGain(-10, 180)).toBe(9);
    expect(yToGain(999, 180)).toBe(-9);
  });

  it('puts the loudest at the top and no change in the middle', () => {
    expect(gainToY(9, 180)).toBe(0);
    expect(gainToY(0, 180)).toBe(90);
    expect(gainToY(-9, 180)).toBe(180);
    expect(yToGain(90, 180)).toBe(0);
    expect(yToGain(42, 180)).toBe(5);
  });

  it('rounds a frequency the way it is told to the user', () => {
    expect(snapFrequency(31.74)).toBe(31.5);
    expect(snapFrequency(99.8)).toBe(100);
    expect(snapFrequency(432.4)).toBe(432);
    expect(snapFrequency(1029.3)).toBe(1030);
    expect(snapFrequency(15994)).toBe(15990);
  });

  it('moves a frequency by one step of the device, within its bounds', () => {
    expect(nudgeFrequency(1000, 1, 500, 2000)).toBe(1030);
    expect(nudgeFrequency(1000, -1, 500, 2000)).toBe(972);
    expect(nudgeFrequency(1990, 1, 500, 2000)).toBe(2000);
    expect(nudgeFrequency(31.5, -1, 31, 63)).toBe(31);
    // A step that rounds to nothing still moves.
    expect(nudgeFrequency(30, 1, 30, 63)).toBeGreaterThan(30);
  });

  it('draws a flat line when nothing is turned up or down', () => {
    const flat = curvePoints(
      [
        { frequency: 100, gain: 0 },
        { frequency: 1000, gain: 0 },
      ],
      720,
      180,
    );
    expect(flat.length).toBeGreaterThan(40);
    expect(flat[0].x).toBe(0);
    expect(flat.at(-1)?.x).toBe(720);
    expect(flat.every((point) => point.y === 90)).toBe(true);
  });

  it('draws the line through every point', () => {
    const bands = [
      { frequency: 31.5, gain: 9 },
      { frequency: 63, gain: 9 },
      { frequency: 125, gain: -9 },
      { frequency: 1000, gain: 4 },
      { frequency: 1030, gain: -2 },
      { frequency: 8000, gain: -6 },
    ];
    const points = curvePoints(bands, 720, 180);
    for (const band of bands) {
      const x = frequencyToX(band.frequency, 720);
      const on = points.find((point) => point.x === x);
      expect(on?.y, `${band.frequency} Hz`).toBeCloseTo(gainToY(band.gain, 180), 6);
    }
    // Left to right, and never above or below what the points ask for.
    expect(points.every((point, at) => at === 0 || point.x >= points[at - 1].x)).toBe(true);
    expect(points.every((point) => point.y >= 0 && point.y <= 180)).toBe(true);
    // Flat beyond the first and the last point.
    expect(points[0]).toEqual({ x: 0, y: 0 });
    expect(points.at(-1)).toEqual({ x: 720, y: gainToY(-6, 180) });
  });

  it('does not overshoot between two points', () => {
    const points = curvePoints(
      [
        { frequency: 100, gain: 0 },
        { frequency: 200, gain: 0 },
        { frequency: 400, gain: 9 },
        { frequency: 800, gain: 9 },
      ],
      720,
      180,
    );
    const before = points.filter((point) => point.x <= frequencyToX(200, 720));
    expect(before.every((point) => Math.abs(point.y - 90) < 1e-6)).toBe(true);
  });

  it('survives two bands at the same frequency, and no band at all', () => {
    const stacked = curvePoints(
      [
        { frequency: 1500, gain: 3 },
        { frequency: 1500, gain: -3 },
      ],
      720,
      180,
    );
    expect(stacked.every((point) => Number.isFinite(point.y))).toBe(true);
    expect(curvePoints([], 720, 180).every((point) => point.y === 90)).toBe(true);
  });
});
