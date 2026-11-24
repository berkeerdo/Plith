// The settings window as a lever frame. Client 840 x 640 DIP plus the 32 DIP caption.
// Sidebar = the box diagram (sections as lever groups); content = one frame per lever group.

const W = 840, H = 640, SIDE = 176, X0 = 204, X1 = 812;

// Lever states across the whole frame, so the sidebar diagram and the content agree.
// t = toggle, c = choice, f = field. s: 'n' normal, 'p' pulled, 'l' locked.
function leverStates(osdOn) {
  return {
    genel: [{ k: 'c' }, { k: 'c' }, { k: 't', s: 'p' }],
    gorunum: [{ k: 'c' }],
    bosta: [{ k: 'c' }, { k: 'c' }, { k: 'c' }, { k: 't', s: 'p' }, { k: 't', s: 'n' }, { k: 'c' }, { k: 'f', s: 'l' }, { k: 'c' }],
    osd: [{ k: 't', s: osdOn ? 'p' : 'n' }, { k: 't', s: osdOn ? 'n' : 'l' }],
  };
}
const SECTIONS = [
  { key: 'genel', name: 'Genel', range: '1-3' },
  { key: 'gorunum', name: 'Görünüm', range: '4' },
  { key: 'bosta', name: 'Boşta', range: '5-12' },
  { key: 'osd', name: 'OSD', range: '13-14' },
  { key: 'hakkinda', name: 'Hakkında', range: 'LEVHA' },
];

// A tiny lever for the box diagram: 1 DIP stem, leaning when pulled, a bar when locked.
function miniLever(svg, x, base, st, active) {
  const pulled = st.k === 'c' || st.s === 'p';
  const color = st.s === 'l' ? C.faint : (pulled ? C.ink : C.faint);
  const top = base - 12, lean = pulled ? 4 : 0;
  add(svg, 'line', { x1: x, y1: base, x2: x + lean, y2: top, stroke: color, 'stroke-width': pulled ? 2 : 1 });
  add(svg, 'rect', { x: x + lean - 1.5, y: top - 3, width: 3, height: 3, fill: pulled ? color : C.floorTop, stroke: color, 'stroke-width': 1 });
  if (st.s === 'l') add(svg, 'rect', { x: x - 3, y: base - 6, width: 7, height: 2, fill: C.ink });
}
function sidebar(svg, active, states) {
  add(svg, 'rect', { x: 0, y: 0, width: SIDE, height: H, fill: C.floorTop });
  line(svg, [[SIDE + 0.5, 0], [SIDE + 0.5, H]], C.border);
  text(svg, 24, 34, 'AYARLAR', STY.label, C.faint);
  text(svg, SIDE - 20, 34, 'KOL 1-14', STY.label, C.faint, 'end');
  let y = 66;
  SECTIONS.forEach(sec => {
    const on = sec.key === active;
    if (on) add(svg, 'rect', { x: 20.5, y: y - 9.5, width: 7, height: 7, fill: C.ink, stroke: C.ink });
    text(svg, 40, y, sec.name, STY.row, on ? C.ink : C.muted);
    text(svg, SIDE - 20, y, sec.range, STY.label, on ? C.ink : C.faint, 'end');
    const levers = states[sec.key];
    if (levers) {
      const base = y + 30;
      const x0 = 42, x1 = x0 + (levers.length - 1) * 10;
      line(svg, [[x0 - 4, base + 0.5], [x1 + 4, base + 0.5]], on ? C.muted : C.track);
      levers.forEach((st, i) => miniLever(svg, x0 + i * 10, base, st, on));
      y += 58;
    } else y += 34;
  });
  // Key to the diagram, at the foot of the sidebar.
  const ky = H - 92;
  line(svg, [[24, ky - 18.5], [SIDE - 20, ky - 18.5]], C.trackFaint);
  [['n', 'NORMAL'], ['p', 'ÇEKİLİ'], ['l', 'KİLİTLİ']].forEach(([s, w], i) => {
    const yy = ky + i * 22;
    line(svg, [[22, yy + 0.5], [34, yy + 0.5]], C.track);
    miniLever(svg, 28, yy, { k: 't', s }, false);
    text(svg, 44, yy, w, STY.label, C.faint);
  });
}
function heading(svg, title, sub, range) {
  text(svg, X0, 50, title, STY.title, C.ink);
  text(svg, X1, 50, range, STY.label, C.faint, 'end');
  text(svg, X0, 72, sub, STY.row, C.muted);
}
function hint(svg, txt) {
  line(svg, [[X0, H - 40.5], [X1, H - 40.5]], C.trackFaint);
  text(svg, X0, H - 20, txt, STY.label, C.faint);
}

// The locking table: Loft's table of offsets, one row per lock.
function lockTable(svg, x, y, w, rows, note) {
  text(svg, x, y + 10, 'KİLİT TABLOSU', STY.label, C.faint);
  const cols = [0, 40, w - 64];
  const hy = y + 44;
  line(svg, [[x, hy + 0.5], [x + w, hy + 0.5]], C.track);
  ['KOL', 'SERBEST İÇİN', 'ŞİMDİ'].forEach((h, i) => text(svg, x + cols[i], hy - 7, h, STY.label, C.faint));
  rows.forEach((r, i) => {
    const ry = hy + 22 + i * 26;
    r.forEach((c, j) => text(svg, x + cols[j], ry, c, j === 2 && c === 'KİLİTLİ' ? STY.labelStrong : STY.label, j === 1 ? C.muted : C.ink));
    line(svg, [[x, ry + 9.5], [x + w, ry + 9.5]], C.trackFaint);
  });
  if (note) para(svg, x, hy + 26 * rows.length + 30, note, STY.body, w, 16, C.muted);
}

// ---- Sections ----
function sectionOsd(svg, osdOn) {
  heading(svg, 'OSD', 'Oyunların üstündeki gösterge ve Windows’un kendi göstergesi.', 'KOL 13-14');
  const bays = [
    { num: 13, name: 'EKRAN GÖSTERGESİ', desc: 'Ses ve parlaklık değişince, oyunların üstünde', kind: 'toggle', state: osdOn ? 'pulled' : 'normal' },
    { num: 14, name: 'WINDOWS GÖSTERGESİNİ GİZLE', desc: 'Plith’in göstergesi açıkken Windows’unkini gizler', kind: 'toggle',
      state: osdOn ? 'normal' : 'locked', lockText: osdOn ? undefined : 'KİLİTLİ: 13 ÇEKİLİ DEĞİL' },
  ];
  frame(svg, X0 - 6, 100, bays, { links: [[13, 14]], unit: 190 });
  lockTable(svg, X0 + 2 * 190 + 18, 100, X1 - (X0 + 2 * 190 + 18),
    [['14', '13 ÇEKİLİ', osdOn ? 'SERBEST' : 'KİLİTLİ']],
    osdOn ? 'Gösterge açık, 14 serbest. 13 normale dönerse 14 de normale iner ve kilitlenir.'
          : 'Windows’un göstergesini gizlemek için önce Plith’in göstergesi açık olmalı. 13’ü çekince 14 serbest kalır.');
  hint(svg, 'KOLA TIKLA YA DA BOŞLUK: ÇEKER · SEKME: SONRAKİ KOL · ESC: KAPATIR');
}
function sectionGenel(svg) {
  heading(svg, 'Genel', 'Dil, adanın nasıl açılacağı, Windows ile başlama.', 'KOL 1-3');
  const bays = [
    { num: 1, units: 2, name: 'DİL', desc: 'Ada, bu pencere ve gösterge bu dilde. Otomatik, Windows dilini izler.', kind: 'choice', cols: 2, pos: 0,
      options: ['OTOMATİK', 'TÜRKÇE', 'ENGLISH', 'DEUTSCH', 'ESPAÑOL', 'FRANÇAIS', 'PORTUGUÊS', 'ITALIANO', 'POLSKI'] },
    { num: 2, name: 'ADA AÇILINCA', desc: 'Tıklayınca ilk görünen sayfa', kind: 'choice', pos: 0, options: ['KARŞILAMA', 'SON SAYFA'], focus: true },
    { num: 3, name: 'WINDOWS İLE BAŞLAT', desc: 'Oturum açınca Plith kendiliğinden açılır', kind: 'toggle', state: 'pulled' },
  ];
  frame(svg, X0 - 6, 100, bays);
  hint(svg, 'KOLA TIKLA YA DA BOŞLUK: ÇEKER · OKLAR: KONUM SEÇER · ESC: KAPATIR');
}

// Small true-to-shape previews of the six resting looks, drawn in ink (no flow colour here).
function lookPreview(svg, x, y, w, kind, picked) {
  const ph = 18, pw = w - 16, px0 = x + 8, py = y + 6;
  if (kind !== 'GİZLİ') add(svg, 'rect', { x: px0 + 0.5, y: py + 0.5, width: pw - 1, height: ph - 1, rx: 8.5, fill: C.floorTop, stroke: picked ? C.ink : C.track });
  const mono9 = { font: MONO, size: 8, weight: 400, track: 0.04 };
  const mid = py + 12;
  if (kind === 'KAROT') { text(svg, px0 + 8, mid, '14:32', mono9, C.ink); text(svg, px0 + pw - 8, mid, 'YAĞMUR 15', mono9, C.muted, 'end'); }
  if (kind === 'SAAT') text(svg, px0 + pw / 2, mid, '14:32', mono9, C.ink, 'middle');
  if (kind === 'ÇALAN') { add(svg, 'rect', { x: px0 + 8, y: py + 7, width: 4, height: 4, fill: C.ink }); text(svg, px0 + 16, mid, 'Nightcall · Kavinsky', { font: SANS, size: 8, weight: 400, track: 0 }, C.muted); }
  if (kind === 'SES') {
    text(svg, px0 + 8, mid, 'G733', mono9, C.ink); text(svg, px0 + pw - 8, mid, '62', mono9, C.ink, 'end');
    line(svg, [[px0 + 34, py + 9], [px0 + pw - 24, py + 9]], C.ink, 1.5);
  }
  if (kind === 'GİZLİ') line(svg, [[px0, py + 1.5], [px0 + pw, py + 1.5]], C.track, 2);
}
function sectionBosta(svg) {
  heading(svg, 'Boşta', 'Ada kapalıyken ne gösterir, tıklayınca ne okursun.', 'KOL 5-12');
  const looks = ['KAROT', 'SAAT', 'ÇALAN', 'SES', 'SADE', 'GİZLİ'];
  const y1 = frame(svg, X0 - 6, 92, [
    { num: 5, units: 4, name: 'KAPALI ADA', desc: 'Ada kapalıyken ne gösterir', descW: 128, kind: 'looks', pos: 0, options: looks, bodyH: 78,
      extra: (s, x, y, w) => {
        const gx = x + 160, cw = (w - 170) / 3;
        looks.forEach((lk, i) => {
          const cx = gx + (i % 3) * cw, cy = y + 8 + Math.floor(i / 3) * 36;
          lookPreview(s, cx, cy, cw - 6, lk, i === 0);
          station(s, cx + 11.5, cy + 32, i === 0);
          text(s, cx + 22, cy + 35.5, lk, i === 0 ? STY.labelStrong : STY.label, i === 0 ? C.ink : C.faint);
        });
      } },
  ]);
  const y2 = frame(svg, X0 - 6, y1 + 12, [
    { num: 6, name: 'ŞERİT', desc: 'Karşılamada kaç saat', kind: 'choice', pos: 1, cols: 2, options: ['±3 SA', '±6 SA', 'TÜM GÜN'] },
    { num: 7, name: 'SAĞ ÜST', desc: 'Karşılamanın köşesi', kind: 'choice', pos: 0, cols: 2, options: ['OTOMATİK', 'HAVA', 'SES', 'ÇALAN', 'BOŞ'] },
    { num: 8, name: 'BUGÜN ÇALANLAR', desc: 'Yalnız bu bilgisayarda', kind: 'toggle', state: 'pulled' },
    { num: 9, name: 'HAVA', desc: 'Konum Open-Meteo’ya gider', kind: 'toggle', state: 'normal' },
  ]);
  const y3 = frame(svg, X0 - 6, y2 + 12, [
    { num: 10, name: 'KONUM', desc: 'Hava için nereden', kind: 'choice', pos: 0, options: ['WINDOWS', 'IP', 'ŞEHİR'] },
    { num: 11, name: 'ŞEHİR', desc: 'Konum Şehir iken', kind: 'field', value: 'Ankara', state: 'locked', lockText: 'KİLİTLİ: 10 ŞEHİR’DE DEĞİL' },
    { num: 12, name: 'BİRİM', desc: 'Sıcaklık', kind: 'choice', pos: 0, options: ['°C', '°F'] },
  ], { links: [[10, 11]] });
  return y3;
}

function settingsWindow(theme, section, opts = {}) {
  useTheme(theme);
  const [box, body] = windowSvg(W, H, 'Plith · Ayarlar', iconInner16(opts.icon || 'plaka'), opts.holder);
  box.dataset.shot = `settings-${theme}-${section}${opts.suffix || ''}`;
  const osdOn = !!opts.osdOn;
  sidebar(body, section, leverStates(osdOn));
  if (section === 'osd') sectionOsd(body, osdOn);
  if (section === 'genel') sectionGenel(body);
  if (section === 'bosta') {
    // Boşta holds eight levers and is taller than the window: its content scrolls under a
    // clip, the sidebar stays. A thin overlay thumb shows where the view is.
    const clip = add(body, 'clipPath', { id: 'clip-' + box.dataset.shot });
    add(clip, 'rect', { x: SIDE + 1, y: 0, width: W - SIDE - 1, height: H });
    const port = add(body, 'g', { 'clip-path': `url(#clip-${box.dataset.shot})` });
    const inner = add(port, 'g');
    const bottom = sectionBosta(inner) + 28;
    const scroll = opts.scroll === 'end' ? Math.max(0, bottom - H) : 0;
    inner.setAttribute('transform', `translate(0 ${-scroll})`);
    const thumbH = H * H / bottom, thumbY = scroll * H / bottom;
    add(body, 'rect', { x: W - 6, y: thumbY + 2, width: 3, height: thumbH - 4, fill: C.track });
    if (opts.scroll) add(body, 'rect', { x: SIDE + 1, y: 0, width: W - SIDE - 1, height: 1, fill: C.border });
  }
  return box;
}
