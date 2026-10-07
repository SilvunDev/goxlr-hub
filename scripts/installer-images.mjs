// Draws the pictures of the Windows installer from the brand files.
//
//   node scripts/installer-images.mjs
//
// It writes the sources as SVG in assets/brand/ and the 24-bit BMP files NSIS
// asks for in crates/app/installer/. Everything is made of the dots of the
// logo, so no font and no dependency is needed. Run it again after a change
// to the logo, and commit what it writes.

import { readFileSync, writeFileSync } from 'node:fs';
import { deflateSync } from 'node:zlib';

const root = new URL('..', import.meta.url);
const brand = new URL('assets/brand/', root);
const installer = new URL('crates/app/installer/', root);

const CASE = '#20241f';
const UNLIT = '#414a40';
const AMBER = '#ffb224';

// The sizes NSIS shows at 100 % scaling.
const SIDEBAR = { width: 164, height: 314 };
const HEADER = { width: 150, height: 57 };

/** Reads the shapes of a brand SVG: rectangles, and circles grouped by fill. */
function shapesOf(file) {
  const svg = readFileSync(new URL(file, brand), 'utf8');
  const number = (tag, name) => Number(tag.match(new RegExp(`\\b${name}="([^"]+)"`))?.[1] ?? 0);
  const text = (tag, name) => tag.match(new RegExp(`\\b${name}="([^"]+)"`))?.[1];
  const shapes = [];
  for (const [tag] of svg.matchAll(/<rect [^>]*>/g)) {
    shapes.push({
      kind: 'rect',
      x: number(tag, 'x'),
      y: number(tag, 'y'),
      width: number(tag, 'width'),
      height: number(tag, 'height'),
      rx: number(tag, 'rx'),
      fill: text(tag, 'fill'),
      stroke: text(tag, 'stroke'),
      strokeWidth: number(tag, 'stroke-width'),
    });
  }
  for (const [, fill, circles] of svg.matchAll(/<g fill="([^"]+)"[^>]*>(.*?)<\/g>/gs)) {
    for (const [tag] of circles.matchAll(/<circle [^>]*>/g)) {
      shapes.push({
        kind: 'circle',
        cx: number(tag, 'cx'),
        cy: number(tag, 'cy'),
        r: number(tag, 'r'),
        fill,
      });
    }
  }
  return shapes;
}

/** Scales shapes, then moves them. */
function place(shapes, scale, dx, dy) {
  return shapes.map((shape) =>
    shape.kind === 'circle'
      ? { ...shape, cx: shape.cx * scale + dx, cy: shape.cy * scale + dy, r: shape.r * scale }
      : {
          ...shape,
          x: shape.x * scale + dx,
          y: shape.y * scale + dy,
          width: shape.width * scale,
          height: shape.height * scale,
          rx: shape.rx * scale,
          strokeWidth: shape.strokeWidth * scale,
        },
  );
}

/**
 * Four level meters in dots, one per fader of the GoXLR. `levels` tells how
 * many dots of each are lit, from the bottom.
 */
function meters({ x, y, pitch, rows, levels, unlit, lit }) {
  const shapes = [];
  levels.forEach((level, meter) => {
    for (let column = 0; column < 3; column += 1) {
      for (let row = 0; row < rows; row += 1) {
        const on = rows - row <= level;
        shapes.push({
          kind: 'circle',
          cx: x + (meter * 5 + column) * pitch,
          cy: y + row * pitch,
          r: on ? lit : unlit,
          fill: on ? AMBER : UNLIT,
        });
      }
    }
  });
  return shapes;
}

const mark = shapesOf('mark.svg');
const logo = shapesOf('logo-dark.svg');
// The wordmark is what the logo holds to the right of the mark: "GoXLR",
// then "Hub" in amber.
const letters = logo.filter((shape) => shape.kind === 'circle' && shape.cx > 64);
const goxlr = letters.filter((shape) => shape.cx < 240);
const hub = letters.filter((shape) => shape.cx > 240);

const sidebar = [
  ...place(mark, 1.25, 42, 30),
  ...place(goxlr, 1, -72.5, 115),
  ...place(hub, 1, -207.5, 153),
  ...meters({ x: 22, y: 236, pitch: 7, rows: 10, levels: [7, 9, 4, 6], unlit: 1.3, lit: 2.2 }),
];

const header = [
  ...meters({ x: 12, y: 13, pitch: 5, rows: 7, levels: [5, 6, 3, 4], unlit: 1, lit: 1.7 }),
  ...place(mark, 0.640625, 101, 8),
];

function svgOf({ width, height }, shapes, label) {
  const round = (value) => Number(value.toFixed(3));
  const body = shapes
    .map((shape) =>
      shape.kind === 'circle'
        ? `<circle cx="${round(shape.cx)}" cy="${round(shape.cy)}" r="${round(shape.r)}" fill="${shape.fill}"/>`
        : `<rect x="${round(shape.x)}" y="${round(shape.y)}" width="${round(shape.width)}" height="${round(shape.height)}" rx="${round(shape.rx)}" fill="${shape.fill ?? 'none'}"${
            shape.stroke
              ? ` stroke="${shape.stroke}" stroke-width="${round(shape.strokeWidth)}"`
              : ''
          }/>`,
    )
    .join('');
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${width} ${height}" width="${width}" height="${height}" role="img" aria-label="${label}"><title>${label}</title><rect width="${width}" height="${height}" fill="${CASE}"/>${body}</svg>\n`;
}

const rgb = (hex) => [1, 3, 5].map((at) => parseInt(hex.slice(at, at + 2), 16));

function inRoundedRect(px, py, x, y, width, height, rx) {
  if (px < x || py < y || px > x + width || py > y + height) return false;
  const radius = Math.max(0, Math.min(rx, width / 2, height / 2));
  const cx = Math.min(Math.max(px, x + radius), x + width - radius);
  const cy = Math.min(Math.max(py, y + radius), y + height - radius);
  return (px - cx) ** 2 + (py - cy) ** 2 <= radius ** 2;
}

function covers(shape, px, py) {
  if (shape.kind === 'circle') {
    return (px - shape.cx) ** 2 + (py - shape.cy) ** 2 <= shape.r ** 2;
  }
  const { x, y, width, height, rx } = shape;
  if (shape.stroke) {
    const half = shape.strokeWidth / 2;
    return (
      inRoundedRect(px, py, x - half, y - half, width + 2 * half, height + 2 * half, rx + half) &&
      !inRoundedRect(px, py, x + half, y + half, width - 2 * half, height - 2 * half, rx - half)
    );
  }
  return inRoundedRect(px, py, x, y, width, height, rx);
}

/** Paints the shapes on the case colour, each pixel averaged over 8 × 8 points. */
function paint({ width, height }, shapes) {
  const SAMPLES = 8;
  const pixels = new Uint8Array(width * height * 3);
  const colours = shapes.map((shape) => rgb(shape.stroke ?? shape.fill));
  const background = rgb(CASE);
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const sum = [0, 0, 0];
      for (let sy = 0; sy < SAMPLES; sy += 1) {
        for (let sx = 0; sx < SAMPLES; sx += 1) {
          const px = x + (sx + 0.5) / SAMPLES;
          const py = y + (sy + 0.5) / SAMPLES;
          let colour = background;
          shapes.forEach((shape, index) => {
            if (covers(shape, px, py)) colour = colours[index];
          });
          sum[0] += colour[0];
          sum[1] += colour[1];
          sum[2] += colour[2];
        }
      }
      const at = (y * width + x) * 3;
      for (let channel = 0; channel < 3; channel += 1) {
        pixels[at + channel] = Math.round(sum[channel] / SAMPLES ** 2);
      }
    }
  }
  return pixels;
}

/** A 24-bit BMP, the only kind NSIS reads: rows bottom-up, blue first. */
function bmpOf({ width, height }, pixels) {
  const row = Math.ceil((width * 3) / 4) * 4;
  const file = Buffer.alloc(54 + row * height);
  file.write('BM');
  file.writeUInt32LE(file.length, 2);
  file.writeUInt32LE(54, 10);
  file.writeUInt32LE(40, 14);
  file.writeInt32LE(width, 18);
  file.writeInt32LE(height, 22);
  file.writeUInt16LE(1, 26);
  file.writeUInt16LE(24, 28);
  file.writeUInt32LE(row * height, 34);
  file.writeInt32LE(2835, 38);
  file.writeInt32LE(2835, 42);
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const from = (y * width + x) * 3;
      const to = 54 + (height - 1 - y) * row + x * 3;
      file[to] = pixels[from + 2];
      file[to + 1] = pixels[from + 1];
      file[to + 2] = pixels[from];
    }
  }
  return file;
}

/** A PNG of the same pixels, to look at the result: `--preview <folder>`. */
function pngOf({ width, height }, pixels) {
  const table = Array.from({ length: 256 }, (_, n) => {
    let c = n;
    for (let k = 0; k < 8; k += 1) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    return c >>> 0;
  });
  const crc = (bytes) => {
    let c = 0xffffffff;
    for (const byte of bytes) c = table[(c ^ byte) & 0xff] ^ (c >>> 8);
    return (c ^ 0xffffffff) >>> 0;
  };
  const chunk = (type, data) => {
    const body = Buffer.concat([Buffer.from(type), data]);
    const out = Buffer.alloc(body.length + 8);
    out.writeUInt32BE(data.length, 0);
    body.copy(out, 4);
    out.writeUInt32BE(crc(body), body.length + 4);
    return out;
  };
  const head = Buffer.alloc(13);
  head.writeUInt32BE(width, 0);
  head.writeUInt32BE(height, 4);
  head[8] = 8;
  head[9] = 2;
  const rows = Buffer.alloc((width * 3 + 1) * height);
  for (let y = 0; y < height; y += 1) {
    Buffer.from(pixels.buffer, y * width * 3, width * 3).copy(rows, y * (width * 3 + 1) + 1);
  }
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', head),
    chunk('IDAT', deflateSync(rows)),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

const preview = process.argv.indexOf('--preview');
const pictures = [
  { name: 'installer-sidebar', size: SIDEBAR, shapes: sidebar, label: 'GoXLR Hub installer sidebar' },
  { name: 'installer-header', size: HEADER, shapes: header, label: 'GoXLR Hub installer header' },
];
for (const { name, size, shapes, label } of pictures) {
  const pixels = paint(size, shapes);
  writeFileSync(new URL(`${name}.svg`, brand), svgOf(size, shapes, label));
  writeFileSync(new URL(`${name.replace('installer-', '')}.bmp`, installer), bmpOf(size, pixels));
  if (preview > 0) {
    writeFileSync(`${process.argv[preview + 1]}/${name}.png`, pngOf(size, pixels));
  }
  console.log(`${name}: ${size.width} x ${size.height}`);
}
