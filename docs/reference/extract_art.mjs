// Extract the top view of the GSX 300 from the manual's SVG and recolour it for a dark UI.
import { readFileSync, writeFileSync } from "node:fs";

const [, , inPath, outPath] = process.argv;
const svg = readFileSync(inPath, "utf8");
const body = svg.slice(svg.indexOf("</defs>") + 7);
const paths = [...body.matchAll(/<path\s[^>]*?\/>/g)].map((m) => m[0]);

function bbox(d) {
  const nums = [...d.matchAll(/-?\d+(?:\.\d+)?/g)].map((m) => parseFloat(m[0]));
  let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
  for (let i = 0; i + 1 < nums.length; i += 2) {
    x0 = Math.min(x0, nums[i]); x1 = Math.max(x1, nums[i]);
    y0 = Math.min(y0, nums[i + 1]); y1 = Math.max(y1, nums[i + 1]);
  }
  return { x0, y0, x1, y1 };
}

const R = { x0: 172, y0: 640, x1: 268, y1: 842 };
const inside = (b) => b.x0 >= R.x0 && b.x1 <= R.x1 && b.y0 >= R.y0 && b.y1 <= R.y1;
const sel = [];
for (const p of paths) {
  const d = /d="([^"]+)"/.exec(p)?.[1];
  if (!d) continue;
  const b = bbox(d);
  if (inside(b)) sel.push({ p, b, d });
}
console.error("selected paths:", sel.length);

const isBlue = (p) => /fill="rgb\(38\.0/.test(p);
const ring = sel.filter((s) => isBlue(s.p));
console.error("ring paths:", ring.map((r) => JSON.stringify(r.b)));
let ringBox = null;
for (const r of ring) {
  ringBox = ringBox
    ? { x0: Math.min(ringBox.x0, r.b.x0), y0: Math.min(ringBox.y0, r.b.y0), x1: Math.max(ringBox.x1, r.b.x1), y1: Math.max(ringBox.y1, r.b.y1) }
    : { ...r.b };
}

// Overall bounds of everything kept (excluding the ring).
const keep = sel.filter((s) => !isBlue(s.p));
let B = { x0: Infinity, y0: Infinity, x1: -Infinity, y1: -Infinity };
for (const s of keep) {
  B = { x0: Math.min(B.x0, s.b.x0), y0: Math.min(B.y0, s.b.y0), x1: Math.max(B.x1, s.b.x1), y1: Math.max(B.y1, s.b.y1) };
}
const pad = 3;
const vb = { x: B.x0 - pad, y: B.y0 - pad, w: B.x1 - B.x0 + 2 * pad, h: B.y1 - B.y0 + 2 * pad };

function recolour(p) {
  return p
    .replace(/fill="rgb\(8\.2[^"]*"/g, 'fill="#8e949e"') // outlines and text
    .replace(/fill="rgb\(100%, 100%, 100%\)"/g, 'fill="#14161a"') // white surfaces
    .replace(/stroke="rgb\(100%, 100%, 100%\)"/g, 'stroke="#14161a"')
    .replace(/fill="rgb\(24\.[^"]*"/g, 'fill="#4a4f58"')
    .replace(/fill="rgb\(68\.[^"]*"/g, 'fill="#3a3e46"')
    .replace(/fill="rgb\(59\.[^"]*"/g, 'fill="#3a3e46"')
    .replace(/fill="rgb\(50\.[^"]*"/g, 'fill="#3a3e46"')
    .replace(/fill="rgb\(90\.[^"]*"/g, 'fill="#14161a"');
}

const out = [
  `<svg xmlns="http://www.w3.org/2000/svg" width="${vb.h.toFixed(2)}" height="${vb.w.toFixed(2)}" viewBox="0 0 ${vb.h.toFixed(2)} ${vb.w.toFixed(2)}">`,
  "<!-- Top view of the GSX 300, traced from the product quick guide, rotated 90 degrees clockwise and recoloured for dark UI. -->",
  `<g transform="translate(${vb.h.toFixed(2)},0) rotate(90) translate(${(-vb.x).toFixed(2)},${(-vb.y).toFixed(2)})">`,
  ...keep.map((s) => recolour(s.p)),
  "</g>",
  "</svg>",
].join("\n");
writeFileSync(outPath, out);

if (ringBox) {
  const cx = (ringBox.x0 + ringBox.x1) / 2, cy = (ringBox.y0 + ringBox.y1) / 2;
  const r = (ringBox.x1 - ringBox.x0) / 2;
  console.log(JSON.stringify({
    viewBox: vb,
    knobFx: (cx - vb.x) / vb.w,
    knobFy: (cy - vb.y) / vb.h,
    ringRFracOfWidth: r / vb.w,
    aspect: vb.w / vb.h,
  }, null, 2));
}
