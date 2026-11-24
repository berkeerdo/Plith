// The installer: a compact window, 560 x 400 DIP plus the caption, three levers.
// step: 'start' (nothing pulled), 'running' (1 and 2 pulled, 3 moving while UAC asks), 'done'.

const IW = 560, IH = 400, IX0 = 24, IX1 = 536;

function button(svg, xRight, y, label, primary) {
  const w = measure(label, STY.labelStrong) + 32, x = xRight - w;
  add(svg, 'rect', { x: x + 0.5, y: y + 0.5, width: w - 1, height: 31, fill: primary ? C.ink : 'none', stroke: primary ? C.ink : C.track });
  text(svg, x + w / 2, y + 20, label, STY.labelStrong, primary ? C.floorTop : C.ink, 'middle');
  return x;
}
const STEPS = {
  start: {
    lede: 'Üç kol var. 1 Plith’i kurar; 2 ve 3 istersen atlanır.',
    states: ['normal', 'normal', 'normal'],
    words: ['NORMAL', 'SIRADA', 'SIRADA'],
    skip: [false, true, true],
    foot: ['KUR’A BASINCA KOLLAR SIRAYLA ÇEKİLİR', 'Bir adım bitmeden sonraki kol çekilmez.'],
    buttons: [['KUR', true], ['VAZGEÇ', false]],
  },
  running: {
    lede: 'Kollar sırayla çekiliyor. Şimdi 3: Windows onay istiyor.',
    states: ['pulled', 'pulled', 'moving'],
    words: ['ÇEKİLİ · KURULDU', 'ÇEKİLİ · EKLENDİ', 'ONAY BEKLİYOR'],
    skip: [false, false, false],
    foot: ['3 / 3 · YARDIMCIYA GÜVEN', 'Onaylarsan imzalı plith-osd.exe Program Files’a kopyalanır. Reddedersen Plith göstergesiz çalışır.'],
    buttons: [],
  },
  done: {
    lede: 'Plith kuruldu. Ada ekranın üst kenarında açılır.',
    states: ['pulled', 'pulled', 'pulled'],
    words: ['ÇEKİLİ · KURULDU', 'ÇEKİLİ · EKLENDİ', 'ÇEKİLİ · GÜVENİLDİ'],
    skip: [false, false, false],
    foot: ['ÜÇ KOL ÇEKİLİ · KURULUM BİTTİ', 'Ayarlar adadaki dişli işaretinden açılır.'],
    buttons: [['PLITH’İ AÇ', true]],
  },
};
function installerWindow(step, icon) {
  useTheme('gece');
  const s = STEPS[step];
  const [box, svg] = windowSvg(IW, IH, 'Plith kurulumu', iconInner16(icon));
  box.dataset.shot = 'installer-' + ({ start: 1, running: 2, done: 3 })[step] + '-window';
  // Header: the icon at 32, the name, the version.
  const g = add(svg, 'g', { transform: `translate(${IX0} 22)` });
  g.innerHTML = iconSvg(icon, 32).replace(/^<svg[^>]*>/, `<svg width="32" height="32" viewBox="0 0 32 32">`);
  text(svg, IX0 + 46, 38, 'Plith', STY.title, C.ink);
  text(svg, IX0 + 46, 54, 'SÜRÜM 0.4.0 · KURULUM', STY.label, C.faint);
  text(svg, IX1, 38, 'KOL 1-3', STY.label, C.faint, 'end');
  text(svg, IX0, 86, s.lede, STY.row, C.muted);
  const names = ['KUR', 'WINDOWS İLE BAŞLAT', 'YARDIMCIYA GÜVEN'];
  // Lever 3 names what UAC will ask before it asks; its line follows the step's tense.
  const third = {
    start: 'Windows, imzalı OSD yardımcısını kurmak için yönetici onayı isteyecek.',
    running: 'Windows şimdi imzalı OSD yardımcısı için yönetici onayı istiyor.',
    done: 'İmzalı OSD yardımcısı yönetici onayıyla kuruldu.',
  }[step];
  const descs = ['Plith’i bu kullanıcıya kurar, Başlat menüsüne ekler.', 'Oturum açınca Plith kendiliğinden açılır.', third];
  const unit = (IX1 - IX0 + 12) / 3;
  const bays = names.map((n, i) => ({
    num: i + 1, name: n, desc: descs[i], kind: 'toggle', state: s.states[i], stateText: s.words[i],
    extra: s.skip[i] ? (sv, x, y, w, h) => {
      const tx = x + w - PAD, ty = y + h - PAD - 2;
      text(sv, tx, ty, 'ATLA', STY.label, C.muted, 'end');
      line(sv, [[tx - measure('ATLA', STY.label), ty + 3.5], [tx, ty + 3.5]], C.muted);
    } : undefined,
  }));
  // A moving lever has an ink state word: it is the one thing happening now.
  frame(svg, IX0 - 6, 100, bays, { unit });
  line(svg, [[0, IH - 68.5], [IW, IH - 68.5]], C.border);
  text(svg, IX0, IH - 44, s.foot[0], STY.labelStrong, C.ink);
  const btnW = s.buttons.reduce((a, [l]) => a + measure(l, STY.labelStrong) + 40, 0);
  para(svg, IX0, IH - 26, s.foot[1], STY.body, IX1 - IX0 - btnW - 16, 15, C.muted);
  let xr = IX1;
  s.buttons.forEach(([label, primary]) => { xr = button(svg, xr, IH - 48, label, primary) - 8; });
  return box;
}
