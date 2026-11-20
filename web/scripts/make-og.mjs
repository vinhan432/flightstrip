// Draw the social card as a PNG, with no image library.
//
// The card is the product's own visual language at 1200x630: paper, a strip with
// hairlines, a trace with one spike, one orange incident mark and the pen head.
// No text is rasterised — there is no font renderer here, and a hand-drawn
// typeface would look worse than the graphic alone. The words come from the
// page's og:title and og:description.
//
// PNG is written by hand (IHDR / IDAT / IEND + CRC32, zlib from stdlib) because
// adding `sharp` or `canvas` for a static asset is not worth a dependency.
//
//   node scripts/make-og.mjs           # writes public/og.png

import { deflateSync } from "node:zlib";
import { writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const out = resolve(here, "..", "public", "og.png");

const WIDTH = 1200;
const HEIGHT = 630;

const PAPER = [0xf4, 0xef, 0xe4];
const PAPER_DARK = [0xe6, 0xdf, 0xd2];
const INK = [0x1c, 0x19, 0x17];
const INK_LIGHT = [0x78, 0x71, 0x6c];
const INK_FAINT = [0xc4, 0xb9, 0xa8];
const ORANGE = [0xd4, 0x62, 0x1a];

const pixels = new Uint8Array(WIDTH * HEIGHT * 3);

function clear(colour) {
  for (let i = 0; i < WIDTH * HEIGHT; i++) {
    pixels[i * 3] = colour[0];
    pixels[i * 3 + 1] = colour[1];
    pixels[i * 3 + 2] = colour[2];
  }
}

function rect(x, y, w, h, colour) {
  const x0 = Math.max(0, Math.round(x));
  const y0 = Math.max(0, Math.round(y));
  const x1 = Math.min(WIDTH, Math.round(x + w));
  const y1 = Math.min(HEIGHT, Math.round(y + h));
  for (let py = y0; py < y1; py++) {
    for (let px = x0; px < x1; px++) {
      const i = (py * WIDTH + px) * 3;
      pixels[i] = colour[0];
      pixels[i + 1] = colour[1];
      pixels[i + 2] = colour[2];
    }
  }
}

function line(x0, y0, x1, y1, colour, thickness = 2) {
  const steps = Math.max(Math.abs(x1 - x0), Math.abs(y1 - y0)) * 2;
  for (let s = 0; s <= steps; s++) {
    const t = steps === 0 ? 0 : s / steps;
    rect(
      x0 + (x1 - x0) * t - thickness / 2,
      y0 + (y1 - y0) * t - thickness / 2,
      thickness,
      thickness,
      colour,
    );
  }
}

function polyline(points, colour, thickness) {
  for (let i = 1; i < points.length; i++) {
    line(points[i - 1][0], points[i - 1][1], points[i][0], points[i][1], colour, thickness);
  }
}

// ── the wordmark ────────────────────────────────────────────────────────────
// Drawn as blocks on a 5x7 grid, the way a chart head is stencilled. Nine
// letters, one colour, no font file.
const GLYPHS = {
  F: ["11111", "10000", "10000", "11110", "10000", "10000", "10000"],
  L: ["10000", "10000", "10000", "10000", "10000", "10000", "11111"],
  I: ["11111", "00100", "00100", "00100", "00100", "00100", "11111"],
  G: ["01110", "10001", "10000", "10111", "10001", "10001", "01110"],
  H: ["10001", "10001", "10001", "11111", "10001", "10001", "10001"],
  T: ["11111", "00100", "00100", "00100", "00100", "00100", "00100"],
  S: ["01111", "10000", "10000", "01110", "00001", "00001", "11110"],
  R: ["11110", "10001", "10001", "11110", "10100", "10010", "10001"],
  P: ["11110", "10001", "10001", "11110", "10000", "10000", "10000"],
};

function wordmark(text, x, y, cell, colour) {
  let cursor = x;
  for (const letter of text) {
    const glyph = GLYPHS[letter];
    if (!glyph) {
      cursor += cell * 3;
      continue;
    }
    for (let row = 0; row < glyph.length; row++) {
      for (let col = 0; col < glyph[row].length; col++) {
        if (glyph[row][col] === "1") rect(cursor + col * cell, y + row * cell, cell, cell, colour);
      }
    }
    cursor += cell * 6;
  }
  return cursor - cell;
}

// ── the sheet ───────────────────────────────────────────────────────────────
clear(PAPER);

// Header band, like the printed head of a chart roll.
rect(0, 0, WIDTH, 96, PAPER_DARK);
rect(0, 96, WIDTH, 2, INK_FAINT);

// Data plate: three short rules standing in for the stamped fields.
rect(64, 40, 220, 6, INK);
rect(64, 56, 132, 4, INK_LIGHT);
rect(WIDTH - 320, 40, 256, 6, INK);
rect(WIDTH - 320, 56, 176, 4, INK_LIGHT);

// The wordmark, then a single rule under it carrying the pen's first mark.
wordmark("FLIGHTSTRIP", 64, 138, 15, INK);
rect(64, 268, WIDTH - 128, 2, INK);
rect(64, 282, 96, 2, ORANGE);

// Strip body hairlines, spaced like a chart's ruled paper.
for (let y = 330; y <= 560; y += 46) {
  rect(64, y, WIDTH - 128, 1, PAPER_DARK);
}
// Time ticks along the bottom edge.
for (let x = 64; x <= WIDTH - 64; x += 30) {
  const tall = (x - 64) % 150 === 0;
  rect(x, 560, 1, tall ? 16 : 8, INK_FAINT);
}
rect(64, 560, WIDTH - 128, 2, INK);

// ── the trace ───────────────────────────────────────────────────────────────
// Quiet, a failing spike, quiet again, then a small recovery: the shape of the
// story the product tells.
const trace = [];
const startX = 96;
const stepX = 24;
const baseline = 470;
const shape = [0, 2, -2, 1, 0, -3, 3, -1, 0, 0, -4, 6, -38, 26, -6, 4, 1, -2, 0, 2, -1, 0, 3, -2, 0, 1, 0, -1, 0, 2];
for (let i = 0; i < shape.length; i++) {
  trace.push([startX + i * stepX, baseline + shape[i]]);
}
polyline(trace, INK_LIGHT, 2);
// The last section is drawn in ink: the part that matters.
polyline(trace.slice(-8), INK, 3);

// Pen head at the end of the trace.
const [penX, penY] = trace[trace.length - 1];
rect(penX - 1.5, 300, 3, 260, INK_FAINT);
line(penX, 300, penX, 560, ORANGE, 2);
rect(penX - 7, penY - 7, 14, 14, ORANGE);

// The one incident mark, where the spike happened.
const spike = trace[12];
rect(spike[0] - 2, 320, 4, 240, ORANGE);
rect(spike[0] - 12, spike[1] - 74, 24, 24, ORANGE);
rect(spike[0] - 6, spike[1] - 68, 12, 12, PAPER);

// ── encode ──────────────────────────────────────────────────────────────────
function crc32(buffer) {
  let table = crc32.table;
  if (!table) {
    table = crc32.table = new Int32Array(256);
    for (let n = 0; n < 256; n++) {
      let c = n;
      for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      table[n] = c;
    }
  }
  let crc = -1;
  for (let i = 0; i < buffer.length; i++) crc = (crc >>> 8) ^ table[(crc ^ buffer[i]) & 0xff];
  return (crc ^ -1) >>> 0;
}

function chunk(type, data) {
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length, 0);
  const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body), 0);
  return Buffer.concat([length, body, crc]);
}

const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(WIDTH, 0);
ihdr.writeUInt32BE(HEIGHT, 4);
ihdr[8] = 8; // bit depth
ihdr[9] = 2; // truecolour RGB
ihdr[10] = 0;
ihdr[11] = 0;
ihdr[12] = 0;

// One filter byte (0 = none) per scanline.
const raw = Buffer.alloc((WIDTH * 3 + 1) * HEIGHT);
for (let y = 0; y < HEIGHT; y++) {
  raw[y * (WIDTH * 3 + 1)] = 0;
  Buffer.from(pixels.buffer, y * WIDTH * 3, WIDTH * 3).copy(raw, y * (WIDTH * 3 + 1) + 1);
}

const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk("IHDR", ihdr),
  chunk("IDAT", deflateSync(raw, { level: 9 })),
  chunk("IEND", Buffer.alloc(0)),
]);

writeFileSync(out, png);
console.log(`make-og: wrote ${out} (${png.length} bytes, ${WIDTH}x${HEIGHT})`);
