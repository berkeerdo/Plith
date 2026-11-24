// Plith app icon, three constructions of one idea: the island at the top edge of a screen
// frame, one pulled lever under it. Each variant is drawn per size class on its own pixel grid:
// a 16 grid for 16 px, a 24 grid for 24 px, a 32 grid for 32, 48 and 256 (more detail at 256).

const IC = { dark: '#1A1916', paper: '#F3F2ED', paperEdge: '#9C978D', night: '#151412', nightEdge: '#5A554C',
  light: '#ECE9E2', faintLight: '#8A847A', faintDark: '#6B665E', lime: '#CAFF33' };

function r(x, y, w, h, fill, extra) { return `<rect x="${x}" y="${y}" width="${w}" height="${h}" fill="${fill}"${extra || ''}/>`; }
function px(cells, fill) { return cells.map(([x, y]) => r(x, y, 1, 1, fill)).join(''); }
function hatchLines(x0, y0, w, h, step, stroke, sw, clipId, clipPath) {
  let s = `<clipPath id="${clipId}"><path d="${clipPath}"/></clipPath><g clip-path="url(#${clipId})">`;
  for (let d = -h; d < w + h; d += step) s += `<line x1="${x0 + d}" y1="${y0 + h}" x2="${x0 + d + h}" y2="${y0}" stroke="${stroke}" stroke-width="${sw}"/>`;
  return s + '</g>';
}
let iconSeq = 0;

// ---- A: Ekran. No tile: the object itself, a dark bezel around a paper screen. ----
function iconEkran(size) {
  if (size === 16) return { grid: 16, crisp: true, body:
    r(1, 2, 14, 12, IC.dark) + r(2, 3, 12, 10, IC.paper) +
    r(5, 3, 6, 1, IC.dark) + r(6, 4, 4, 1, IC.dark) +                 // island, bottom corners cut
    r(3, 11, 3, 2, IC.dark) + px([[6, 10], [7, 9], [8, 8]], IC.dark) + // pivot block and stem
    r(9, 6, 2, 2, IC.dark) };                                          // handle
  if (size === 24) return { grid: 24, crisp: true, body:
    r(1, 3, 22, 18, IC.dark) + r(3, 5, 18, 14, IC.paper) +
    r(8, 5, 8, 2, IC.dark) + r(9, 7, 6, 1, IC.dark) +
    r(5, 16, 4, 3, IC.dark) +
    px([[9, 15], [10, 14], [11, 13], [12, 12], [9, 16], [10, 15], [11, 14], [12, 13]], IC.dark) +
    r(13, 10, 3, 3, IC.dark) };
  const big = size >= 128, id = 'e' + (iconSeq++);
  let s = r(2, 4, 28, 23, IC.dark) + r(4, 6, 24, 19, IC.paper) +
    `<path d="M11 6H21V8Q21 9.5 19.5 9.5H12.5Q11 9.5 11 8Z" fill="${IC.dark}"/>` +
    `<line x1="6" y1="22.5" x2="26" y2="22.5" stroke="${IC.dark}" stroke-width="1"/>` +
    r(7.5, 20.5, 4, 4.5, IC.dark) +
    `<line x1="10.5" y1="21" x2="16.2" y2="14.2" stroke="${IC.dark}" stroke-width="${big ? 1.8 : 2.2}"/>` +
    r(big ? 15.25 : 15, big ? 11.25 : 11, big ? 3.5 : 4, big ? 3.5 : 4, IC.lime, ` stroke="${IC.dark}" stroke-width="1"`);
  if (big) {
    for (let x = 13; x <= 25; x += 2) s += `<line x1="${x}" y1="22.5" x2="${x}" y2="${x % 4 === 1 ? 24.2 : 23.4}" stroke="${IC.dark}" stroke-width=".25"/>`;
    s += r(25.25, 21.75, 1.5, 1.5, IC.paper, ` stroke="${IC.dark}" stroke-width=".3"`);
  }
  return { grid: 32, crisp: false, body: s, id };
}

// ---- B: Plaka. A night tile is the screen; the island hangs from its top edge in ink. ----
function iconPlaka(size) {
  if (size === 16) return { grid: 16, crisp: true, body:
    r(0, 0, 16, 16, IC.nightEdge) + r(1, 1, 14, 14, IC.night) +
    r(4, 1, 8, 2, IC.light) + r(5, 3, 6, 1, IC.light) +
    r(2, 13, 12, 1, IC.faintLight) + r(4, 11, 3, 2, IC.light) +
    px([[7, 10], [8, 9], [9, 8]], IC.light) + r(10, 6, 2, 2, IC.lime) };
  if (size === 24) return { grid: 24, crisp: true, body:
    r(0, 0, 24, 24, IC.nightEdge) + r(1, 1, 22, 22, IC.night) +
    r(7, 1, 10, 3, IC.light) + r(8, 4, 8, 1, IC.light) +
    r(3, 19, 18, 1, IC.faintLight) + r(6, 16, 4, 3, IC.light) +
    px([[10, 15], [11, 14], [12, 13], [13, 12], [10, 16], [11, 15], [12, 14], [13, 13]], IC.light) +
    r(14, 9, 3, 3, IC.lime) };
  const big = size >= 128;
  let s = r(0, 0, 32, 32, IC.nightEdge) + r(1, 1, 30, 30, IC.night) +
    `<path d="M10 1H22V4.5Q22 7 19.5 7H12.5Q10 7 10 4.5Z" fill="${IC.light}"/>` +
    `<line x1="5" y1="25.5" x2="27" y2="25.5" stroke="${IC.faintLight}" stroke-width="1"/>` +
    r(7, 22.5, 5, 4, IC.light) +
    `<line x1="10.5" y1="23" x2="17.6" y2="13.6" stroke="${IC.light}" stroke-width="${big ? 1.9 : 2.4}"/>` +
    r(big ? 15.75 : 15.5, big ? 11.5 : 11, big ? 3.5 : 4.5, big ? 3.5 : 4.5, IC.lime);
  if (big) {
    for (let x = 13; x <= 27; x += 2) s += `<line x1="${x}" y1="25.5" x2="${x}" y2="${x % 4 === 1 ? 27.4 : 26.6}" stroke="${IC.faintLight}" stroke-width=".25"/>`;
    s += r(26.25, 24.75, 1.5, 1.5, IC.night, ` stroke="${IC.faintLight}" stroke-width=".3"`);
    s += r(4.25, 24.75, 1.5, 1.5, IC.night, ` stroke="${IC.faintLight}" stroke-width=".3"`);
  }
  return { grid: 32, crisp: false, body: s };
}

// ---- C: Kesit. A paper tile; the island is drawn as a hatched section, the pivot too. ----
function iconKesit(size) {
  if (size === 16) return { grid: 16, crisp: true, body:
    r(0, 0, 16, 16, IC.paperEdge) + r(1, 1, 14, 14, IC.paper) + r(1, 1, 14, 1, IC.dark) +
    r(4, 2, 8, 1, IC.dark) + r(5, 3, 6, 1, IC.dark) +
    r(2, 13, 12, 1, IC.dark) + r(4, 11, 3, 2, IC.dark) +
    px([[7, 10], [8, 9], [9, 8]], IC.dark) + r(10, 6, 2, 2, IC.dark) };
  if (size === 24) {
    return { grid: 24, crisp: true, body:
      r(0, 0, 24, 24, IC.paperEdge) + r(1, 1, 22, 22, IC.paper) + r(1, 1, 22, 1, IC.dark) +
      r(7, 2, 10, 3, IC.dark) + r(8, 5, 8, 1, IC.dark) +
      r(3, 19, 18, 1, IC.dark) + px([[9, 20], [13, 20], [17, 20]], IC.dark) + r(6, 16, 4, 3, IC.dark) +
      px([[10, 15], [11, 14], [12, 13], [13, 12], [10, 16], [11, 15], [12, 14], [13, 13]], IC.dark) +
      r(14, 9, 3, 3, IC.dark) };
  }
  const big = size >= 128, id = 'k' + (iconSeq++);
  const isl = 'M9.5 1.5H22.5V5Q22.5 7.5 20 7.5H12Q9.5 7.5 9.5 5Z';
  let s = r(0, 0, 32, 32, IC.paperEdge) + r(1, 1, 30, 30, IC.paper) +
    `<line x1="1" y1="1.5" x2="31" y2="1.5" stroke="${IC.dark}" stroke-width="1"/>` +
    hatchLines(9.5, 1.5, 13, 6, big ? 1.4 : 2.2, IC.dark, big ? 0.35 : 0.7, id + 'i', isl) +
    `<path d="${isl}" fill="none" stroke="${IC.dark}" stroke-width="1.2"/>` +
    `<line x1="4" y1="25.5" x2="28" y2="25.5" stroke="${IC.dark}" stroke-width="1"/>`;
  for (let x = 4; x <= 28; x += big ? 2 : 4) s += `<line x1="${x}" y1="25.5" x2="${x}" y2="${(x - 4) % 8 === 0 ? 28 : 27}" stroke="${IC.dark}" stroke-width="${big ? 0.3 : 0.8}"/>`;
  const piv = 'M7 21.5H12V25.5H7Z';
  s += hatchLines(7, 21.5, 5, 4, big ? 1.2 : 1.8, IC.dark, big ? 0.3 : 0.6, id + 'p', piv) +
    `<path d="${piv}" fill="none" stroke="${IC.dark}" stroke-width="1"/>` +
    `<line x1="10.5" y1="22" x2="17.4" y2="13.6" stroke="${IC.dark}" stroke-width="${big ? 1.9 : 2.4}"/>` +
    r(big ? 15.75 : 15.5, big ? 11.5 : 11, big ? 3.5 : 4.5, big ? 3.5 : 4.5, IC.lime,` stroke="${IC.dark}" stroke-width="1"`);
  if (big) s += `<path d="M9.5 16A8 8 0 0 1 16.2 18.4" fill="none" stroke="${IC.dark}" stroke-width=".3"/>`;
  return { grid: 32, crisp: false, body: s };
}

const ICON_VARIANTS = {
  ekran: { name: 'A · EKRAN', fn: iconEkran, note: 'Karo yok: nesnenin kendisi. Koyu çerçeve, kağıt ekran, üst kenarda ada, altında çekili kol.' },
  plaka: { name: 'B · PLAKA', fn: iconPlaka, note: 'Gece karosu ekranın kendisi; ada üst kenardan mürekkeple sarkar, kol rayın üstünde.' },
  kesit: { name: 'C · KESİT', fn: iconKesit, note: 'Kağıt karo, Loft kesit çizimi: ada ve kol ayağı taralı kesit, ray cetvel çentikli.' },
};

// Full SVG markup of one variant at one size.
function iconSvg(variant, size) {
  const d = ICON_VARIANTS[variant].fn(size);
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 ${d.grid} ${d.grid}"${d.crisp ? ' shape-rendering="crispEdges"' : ''}>${d.body}</svg>`;
}
// Inner markup for embedding at 16 DIP inside another SVG (the window caption).
function iconInner16(variant) {
  const d = ICON_VARIANTS[variant].fn(16);
  return `<g shape-rendering="crispEdges">${d.body}</g>`;
}
