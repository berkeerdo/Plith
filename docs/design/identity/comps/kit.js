// Shared drawing kit for the Kol Sirasi comps: palettes, SVG helpers, text wrap, the lever,
// the plate, the frame and the Windows caption. Everything is drawn at 1:1 DIP.

const THEMES = {
  gece: { floorTop: '#0B0B0A', floorBottom: '#151412', border: '#2B2824', ink: '#ECE9E2', muted: '#A39E94',
    faint: '#8A847A', track: '#4A463F', trackFaint: '#2E2B27', plate: '#121110', dark: true },
  kagit: { floorTop: '#F3F2ED', floorBottom: '#E7E6E0', border: '#CFCBC2', ink: '#1A1916', muted: '#55514A',
    faint: '#6B665E', track: '#B3AEA4', trackFaint: '#D6D2C9', plate: '#EEEDE7', dark: false },
};
let C = THEMES.gece;
function useTheme(key) { C = THEMES[key]; }

const MONO = '"Martian Mono", monospace';
const SANS = '"Public Sans", sans-serif';
const SVGNS = 'http://www.w3.org/2000/svg';

function el(tag, attrs, text) {
  const n = document.createElementNS(SVGNS, tag);
  for (const k in attrs) if (attrs[k] !== undefined) n.setAttribute(k, attrs[k]);
  if (text !== undefined) n.textContent = text;
  return n;
}
function add(parent, tag, attrs, text) { const n = el(tag, attrs, text); parent.appendChild(n); return n; }

// Text styles. Mono labels are upper case with tracking; sans is sentence case.
const STY = {
  label: { font: MONO, size: 10, weight: 400, track: 0.06 },
  labelStrong: { font: MONO, size: 10, weight: 500, track: 0.06 },
  num: { font: MONO, size: 13, weight: 500, track: 0.02 },
  body: { font: SANS, size: 12, weight: 400, track: 0 },
  row: { font: SANS, size: 13, weight: 400, track: 0 },
  title: { font: SANS, size: 20, weight: 600, track: 0 },
};
const measureCtx = document.createElement('canvas').getContext('2d');
function measure(txt, s) {
  measureCtx.font = `${s.weight} ${s.size}px ${s.font}`;
  measureCtx.letterSpacing = `${s.track * s.size}px`;
  return measureCtx.measureText(txt).width;
}
function wrap(txt, s, width) {
  const words = txt.split(' '), lines = [];
  let cur = '';
  for (const w of words) {
    const t = cur ? cur + ' ' + w : w;
    if (measure(t, s) <= width || !cur) cur = t; else { lines.push(cur); cur = w; }
  }
  if (cur) lines.push(cur);
  return lines;
}
function text(svg, x, y, txt, s, fill, anchor) {
  return add(svg, 'text', { x, y, fill: fill || C.ink, 'font-family': s.font, 'font-size': s.size,
    'font-weight': s.weight, 'letter-spacing': s.track ? `${s.track}em` : undefined, 'text-anchor': anchor || 'start',
    'font-variant-numeric': s.font === MONO ? 'tabular-nums' : undefined }, txt);
}
// Draws wrapped lines, returns the baseline of the last line.
function para(svg, x, y, txt, s, width, lh, fill) {
  const lines = wrap(txt, s, width);
  lines.forEach((l, i) => text(svg, x, y + i * lh, l, s, fill));
  return y + (lines.length - 1) * lh;
}
function line(svg, pts, stroke, w, dash) {
  return add(svg, 'polyline', { points: pts.map(p => p.join(',')).join(' '), fill: 'none', stroke,
    'stroke-width': w || 1, 'stroke-dasharray': dash, 'stroke-linejoin': 'miter', 'shape-rendering': 'crispEdges' });
}
// A station square: 7 DIP with a 1 DIP stroke, filled ink or hollow.
function station(svg, x, y, filled, color) {
  const c = color || (filled ? C.ink : C.faint);
  add(svg, 'rect', { x: x - 3.5, y: y - 3.5, width: 7, height: 7, fill: filled ? c : C.floorTop, stroke: c, 'stroke-width': 1 });
}
let hatchId = 0;
// Loft section hatch: -45 degree hairlines, inside a rect, drawn as a pattern.
function hatch(svg, x, y, w, h, color, gap) {
  const id = 'h' + (hatchId++);
  const p = add(svg, 'pattern', { id, width: gap || 5, height: gap || 5, patternUnits: 'userSpaceOnUse', patternTransform: 'rotate(45)' });
  add(p, 'line', { x1: 0, y1: 0, x2: 0, y2: gap || 5, stroke: color, 'stroke-width': 1 });
  add(svg, 'rect', { x, y, width: w, height: h, fill: `url(#${id})` });
}

// ---- The lever ----
// state: 'normal' (upright, hollow), 'pulled' (leaning toward you, filled ink), 'locked'
// (hollow and faint, a hatched lock bar across it), 'moving' (part way, hollow with ink edge).
// A choice lever (o.choice) is always filled: it is never off, only set to a notch.
const LEVER = { arcR: 40, stem: 58, sweep: 30 };
function polar(cx, cy, r, deg) { const a = deg * Math.PI / 180; return [cx + r * Math.sin(a), cy - r * Math.cos(a)]; }
function lever(svg, cx, cy, o) {
  const k = o.positions || 2;
  const angles = Array.from({ length: k }, (_, i) => (k === 1 ? 0 : i * LEVER.sweep / (k - 1)));
  // Quadrant: an arc from the first notch to the last, every notch a tick, the set one in ink.
  const [ax0, ay0] = polar(cx, cy, LEVER.arcR, angles[0]);
  const [ax1, ay1] = polar(cx, cy, LEVER.arcR, angles[k - 1]);
  add(svg, 'path', { d: `M${ax0} ${ay0} A${LEVER.arcR} ${LEVER.arcR} 0 0 1 ${ax1} ${ay1}`, fill: 'none', stroke: C.track, 'stroke-width': 1 });
  let a;
  if (o.state === 'moving') a = LEVER.sweep * 0.5;
  else if (!o.choice) a = o.state === 'pulled' ? angles[k - 1] : angles[0];
  else a = angles[o.pos || 0];
  angles.forEach(ang => {
    const [x0, y0] = polar(cx, cy, LEVER.arcR - 4, ang);
    const [x1, y1] = polar(cx, cy, LEVER.arcR + 5, ang);
    const set = Math.abs(ang - a) < 0.01 && o.state !== 'locked';
    add(svg, 'line', { x1: x0, y1: y0, x2: x1, y2: y1, stroke: set ? C.ink : C.faint, 'stroke-width': set ? 2 : 1 });
  });
  const filled = o.state === 'pulled' || (o.choice && o.state !== 'locked');
  const color = o.state === 'locked' ? C.faint : C.ink;
  // Stem as a 5 DIP bar, catch handle as a 10 x 12 block at its end, rotated about the pivot.
  const g = add(svg, 'g', { transform: `rotate(${a} ${cx} ${cy})` });
  add(g, 'rect', { x: cx - 2.5, y: cy - LEVER.stem, width: 5, height: LEVER.stem - 4, fill: filled ? C.ink : C.floorTop, stroke: color, 'stroke-width': 1 });
  add(g, 'rect', { x: cx - 5.5, y: cy - LEVER.stem - 12, width: 11, height: 13, fill: filled ? C.ink : C.floorTop, stroke: color, 'stroke-width': 1 });
  // Pivot on the rail.
  add(svg, 'rect', { x: cx - 5.5, y: cy - 5.5, width: 11, height: 11, fill: C.floorTop, stroke: color, 'stroke-width': 1 });
  if (o.state === 'locked') {
    // The lock bar: a hatched tappet across the stem, edged in ink.
    const bx = cx - 17, by = cy - 26, bw = 34, bh = 9;
    add(svg, 'rect', { x: bx, y: by, width: bw, height: bh, fill: C.floorTop });
    hatch(svg, bx, by, bw, bh, C.ink, 4);
    add(svg, 'rect', { x: bx + 0.5, y: by + 0.5, width: bw - 1, height: bh - 1, fill: 'none', stroke: C.ink, 'stroke-width': 1 });
  }
}

// ---- The frame: plates over levers, one rail, the locking bed under it ----
// bays: [{ units, num, name, desc, kind: 'toggle'|'choice'|'field'|'looks', state, pos, options,
//          lockText, value, focus, bodyH, descW, extra(svg, x, y, w, h) }]
const BAY = 152, PAD = 10;
// Choice positions flow left to right and wrap: [{ x, row }] relative to the plate's text column.
function flow(options, pos, width) {
  let x = 0, row = 0;
  return options.map((o, i) => {
    const w = 14 + measure(o, i === pos ? STY.labelStrong : STY.label);
    if (x > 0 && x + w > width) { x = 0; row++; }
    const at = { x, row };
    x += w + 12;
    return at;
  });
}
function stateLines(b, w) {
  if (b.lockText) return wrap(b.lockText, STY.labelStrong, w - 2 * PAD - 14).length;
  return b.kind === 'toggle' ? 1 : 0;
}
function plateHeight(b, w) {
  let h = PAD + 13;                                   // number and name row
  h += (wrap(b.name, STY.labelStrong, w - 2 * PAD - 28).length - 1) * 14;
  if (b.desc) h += 8 + wrap(b.desc, STY.body, b.descW || (w - 2 * PAD)).length * 16;
  if (b.kind === 'choice') {
    const f = flow(b.options, b.pos, w - 2 * PAD);
    h += 8 + (f[f.length - 1].row + 1) * 17;
  }
  if (b.kind === 'field') h += 36;
  const sl = stateLines(b, w);
  if (sl) h += 12 + sl * 16;
  if (b.bodyH) h = Math.max(h, b.bodyH);
  return h + PAD;
}
function drawPlate(svg, b, x, y, w, h) {
  const locked = b.state === 'locked';
  add(svg, 'rect', { x: x + 0.5, y: y + 0.5, width: w - 1, height: h - 1, fill: C.plate, stroke: locked ? C.trackFaint : C.border, 'stroke-width': 1 });
  if (b.focus) add(svg, 'rect', { x: x - 3.5, y: y - 3.5, width: w + 7, height: h + LEVER_AREA + 7, fill: 'none', stroke: C.ink, 'stroke-width': 1 });
  const ink = locked ? C.faint : C.ink;
  text(svg, x + PAD, y + PAD + 11, String(b.num), STY.num, ink);
  const nameLines = wrap(b.name, STY.labelStrong, w - 2 * PAD - 28);
  nameLines.forEach((l, i) => text(svg, x + PAD + 28, y + PAD + 10 + i * 14, l, STY.labelStrong, ink));
  let cy = y + PAD + 10 + (nameLines.length - 1) * 14;
  if (b.desc) cy = para(svg, x + PAD, cy + 8 + 14, b.desc, STY.body, b.descW || (w - 2 * PAD), 16, locked ? C.faint : C.muted);
  if (b.kind === 'choice') {
    const f = flow(b.options, b.pos, w - 2 * PAD);
    b.options.forEach((opt, i) => {
      const ox = x + PAD + f[i].x, oy = cy + 8 + 13 + f[i].row * 17;
      const picked = i === b.pos;
      station(svg, ox + 3.5, oy - 4, picked);
      text(svg, ox + 14, oy, opt, picked ? STY.labelStrong : STY.label, picked ? C.ink : C.faint);
    });
  }
  if (b.kind === 'field') {
    const fy = cy + 10;
    add(svg, 'rect', { x: x + PAD + 0.5, y: fy + 0.5, width: w - 2 * PAD - 1, height: 25, fill: 'none', stroke: locked ? C.trackFaint : C.track });
    text(svg, x + PAD + 8, fy + 17, b.value, STY.body, locked ? C.faint : C.ink);
  }
  // The state line sits at the foot of the plate: the lever's words, or why it is locked.
  const sl = stateLines(b, w);
  if (!sl) { if (b.extra) b.extra(svg, x, y, w, h); return; }
  const base = y + h - PAD - 2 - (sl - 1) * 16;
  if (b.lockText) {
    const mx = x + PAD, my = base - 8;
    hatch(svg, mx, my, 8, 8, C.ink, 3);
    add(svg, 'rect', { x: mx + 0.5, y: my + 0.5, width: 7, height: 7, fill: 'none', stroke: C.ink });
    wrap(b.lockText, STY.labelStrong, w - 2 * PAD - 14).forEach((l, i) => text(svg, x + PAD + 14, base + i * 16, l, STY.labelStrong, C.ink));
  } else {
    const on = b.state === 'pulled' || b.state === 'moving';
    text(svg, x + PAD, base, b.stateText || (on ? 'ÇEKİLİ · AÇIK' : 'NORMAL · KAPALI'), on ? STY.labelStrong : STY.label, on ? C.ink : C.faint);
  }
  if (b.extra) b.extra(svg, x, y, w, h);
}
const LEVER_AREA = 84, BED = 8;
// Returns the frame's bottom y. opts.links: [[fromNum, toNum]] interlocks drawn in the bed.
function frame(svg, x, y, bays, opts = {}) {
  const unit = opts.unit || BAY;
  const plateH = Math.max(...bays.map(b => plateHeight(b, (b.units || 1) * unit - 12)));
  const railY = y + plateH + LEVER_AREA;
  const pivots = {};
  let bx = x;
  bays.forEach(b => {
    const w = (b.units || 1) * unit;
    drawPlate(svg, b, bx + 6, y, w - 12, plateH);
    pivots[b.num] = bx + unit / 2;
    bx += w;
  });
  const x0 = x + 6, x1 = bx - 6;
  // Locking bed under the rail: the hatched section, closed top and bottom.
  hatch(svg, x0, railY, x1 - x0, BED, C.trackFaint, 5);
  line(svg, [[x0, railY + BED], [x1, railY + BED]], C.trackFaint);
  line(svg, [[x0, railY], [x1, railY]], C.track);
  station(svg, x0, railY, false, C.track); station(svg, x1, railY, false, C.track);
  (opts.links || []).forEach(([a, b]) => {
    const ly = railY + BED + 8;
    line(svg, [[pivots[a], railY + 6], [pivots[a], ly], [pivots[b], ly], [pivots[b], railY + 6]], C.ink);
    add(svg, 'rect', { x: pivots[a] - 2.5, y: ly - 2.5, width: 5, height: 5, fill: C.ink });
    add(svg, 'rect', { x: pivots[b] - 2.5, y: ly - 2.5, width: 5, height: 5, fill: C.ink });
  });
  bays.forEach(b => {
    if (b.noLever) return;
    const choice = b.kind === 'choice' || b.kind === 'looks';
    lever(svg, pivots[b.num], railY, { state: b.state, choice, positions: choice ? b.options.length : 2, pos: b.pos });
  });
  return railY + BED + ((opts.links || []).length ? 14 : 0);
}


// ---- Windows 11 caption, 32 DIP, dark or light, with the app icon at 16 ----
function caption(svg, w, title, icon16, noMax) {
  const bg = C.dark ? '#1F1F1F' : '#F3F3F3', fg = C.dark ? '#FFFFFF' : '#1A1A1A', dim = C.dark ? '#7A7A7A' : '#A0A0A0';
  add(svg, 'rect', { x: 0, y: 0, width: w, height: 32, fill: bg });
  if (icon16) { const g = add(svg, 'g', { transform: 'translate(12 8)' }); g.innerHTML = icon16; }
  add(svg, 'text', { x: 40, y: 20.5, fill: fg, 'font-family': '"Segoe UI Variable Text", "Segoe UI", sans-serif', 'font-size': 12 }, title);
  // Minimize, maximize (disabled: the window is not resizable), close: 46 DIP buttons.
  const bx = w - 138;
  line(svg, [[bx + 18, 16.5], [bx + 28, 16.5]], fg);
  add(svg, 'rect', { x: bx + 46 + 18.5, y: 11.5, width: 9, height: 9, fill: 'none', stroke: noMax ? dim : fg });
  add(svg, 'path', { d: `M${bx + 92 + 18} 11l10 10M${bx + 92 + 28} 11l-10 10`, stroke: fg, 'stroke-width': 1 });
}
function windowSvg(w, h, title, icon16, holder) {
  const svg = el('svg', { width: w, height: h + 32, viewBox: `0 0 ${w} ${h + 32}` });
  const box = document.createElement('div');
  box.className = 'win';
  box.style.width = w + 'px';
  box.appendChild(svg);
  (holder || document.body).appendChild(box);
  caption(svg, w, title, icon16, true);
  const grad = add(svg, 'linearGradient', { id: 'floor' + hatchId, x1: 0, y1: 0, x2: 0, y2: 1 });
  add(grad, 'stop', { offset: 0, 'stop-color': C.floorTop });
  add(grad, 'stop', { offset: 1, 'stop-color': C.floorBottom });
  add(svg, 'rect', { x: 0, y: 32, width: w, height: h, fill: `url(#floor${hatchId})` });
  hatchId++;
  const body = add(svg, 'g', { transform: 'translate(0 32)' });
  return [box, body];
}

// Wait for the two faces before measuring any text, then build, then flag the page as ready.
function whenFonts(build) {
  Promise.all(['400 10px "Martian Mono"', '500 10px "Martian Mono"', '400 12px "Public Sans"', '600 20px "Public Sans"']
    .map(f => document.fonts.load(f, 'AĞİŞ'))).then(() => { build(); document.body.dataset.ready = '1'; });
}
