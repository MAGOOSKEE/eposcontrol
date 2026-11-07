// Cut a region out of a poppler SVG (pdftocairo -svg) and recolour it for a dark UI.
// Usage: node extract_vector.mjs in.svg out.svg x0,y0,x1,y1 [rotate90] [dropBlue]
//   region is in PDF points; only paths fully inside it are kept.
import { readFileSync, writeFileSync } from "node:fs";

const [, , inPath, outPath, regionArg, ...flags] = process.argv;
const [rx0, ry0, rx1, ry1] = regionArg.split(",").map(Number);
const rotate = flags.includes("rotate90");
const dropBlue = flags.includes("dropBlue");

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

const colorRe = /(fill|stroke)="rgb\(([\d.]+)%, ([\d.]+)%, ([\d.]+)%\)"/g;
function recolour(p) {
  return p.replace(colorRe, (_, attr, r, g, b) => {
    const [R, G, B] = [r, g, b].map((v) => parseFloat(v) / 100);
    const lum = 0.2126 * R + 0.7152 * G + 0.0722 * B;
    const sat = Math.max(R, G, B) - Math.min(R, G, B);
    if (sat > 0.15) return `${attr}="#4a5a66"`; // coloured highlights become a quiet slate
    if (lum < 0.25) return `${attr}="#8e949e"`; // outlines and text
    if (lum > 0.85) return `${attr}="#14161a"`; // white surfaces
    return `${attr}="#2c3036"`;
  });
}

const keep = [];
let ring = null;
for (const p of paths) {
  const d = /d="([^"]+)"/.exec(p)?.[1];
  if (!d) continue;
  const b = bbox(d);
  if (b.x0 < rx0 || b.x1 > rx1 || b.y0 < ry0 || b.y1 > ry1) continue;
  const blue = /fill="rgb\(38\.0/.test(p);
  if (blue && dropBlue) {
    ring = ring
      ? { x0: Math.min(ring.x0, b.x0), y0: Math.min(ring.y0, b.y0), x1: Math.max(ring.x1, b.x1), y1: Math.max(ring.y1, b.y1) }
      : { ...b };
    continue;
  }
  keep.push({ p, b });
}

let B = { x0: Infinity, y0: Infinity, x1: -Infinity, y1: -Infinity };
for (const k of keep) {
  B = { x0: Math.min(B.x0, k.b.x0), y0: Math.min(B.y0, k.b.y0), x1: Math.max(B.x1, k.b.x1), y1: Math.max(B.y1, k.b.y1) };
}
const pad = 3;
const vb = { x: B.x0 - pad, y: B.y0 - pad, w: B.x1 - B.x0 + 2 * pad, h: B.y1 - B.y0 + 2 * pad };
const [W, H] = rotate ? [vb.h, vb.w] : [vb.w, vb.h];
const tf = rotate
  ? `translate(${vb.h.toFixed(2)},0) rotate(90) translate(${(-vb.x).toFixed(2)},${(-vb.y).toFixed(2)})`
  : `translate(${(-vb.x).toFixed(2)},${(-vb.y).toFixed(2)})`;
const out = [
  `<svg xmlns="http://www.w3.org/2000/svg" width="${W.toFixed(2)}" height="${H.toFixed(2)}" viewBox="0 0 ${W.toFixed(2)} ${H.toFixed(2)}">`,
  "<!-- Traced from the official product guide and recoloured for dark UI. -->",
  `<g transform="${tf}">`,
  ...keep.map((k) => recolour(k.p)),
  "</g>",
  "</svg>",
].join("\n");
writeFileSync(outPath, out);

const info = { paths: keep.length, width: W, height: H, aspect: W / H };
if (ring) {
  const cx = (ring.x0 + ring.x1) / 2, cy = (ring.y0 + ring.y1) / 2;
  const r = (ring.x1 - ring.x0) / 2;
  if (rotate) {
    info.knobFx = 1 - (cy - vb.y) / vb.h;
    info.knobFy = (cx - vb.x) / vb.w;
    info.ringRFracOfWidth = r / vb.h;
  } else {
    info.knobFx = (cx - vb.x) / vb.w;
    info.knobFy = (cy - vb.y) / vb.h;
    info.ringRFracOfWidth = r / vb.w;
  }
}
console.log(JSON.stringify(info));
