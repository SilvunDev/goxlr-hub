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

  it('bends the line towards a band that is turned up, and stays in the drawing', () => {
    const points = curvePoints(
      [
        { frequency: 1000, gain: 9 },
        { frequency: 1200, gain: 9 },
        { frequency: 8000, gain: -6 },
      ],
      720,
      180,
    );
    const nearest = (hertz: number) => {
      const x = frequencyToX(hertz, 720);
      return points.reduce((best, point) =>
        Math.abs(point.x - x) < Math.abs(best.x - x) ? point : best,
      );
    };
    expect(nearest(1000).y).toBeLessThan(30);
    expect(nearest(8000).y).toBeGreaterThan(120);
    expect(nearest(40).y).toBeCloseTo(90, 0);
    expect(points.every((point) => point.y >= 0 && point.y <= 180)).toBe(true);
  });
});
