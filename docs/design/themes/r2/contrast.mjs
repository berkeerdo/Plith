// WCAG contrast of each theme's ink colours against its measured floor mean.
const themes = {
  halka: { floor: "#C7F910", ink: "#141A05", muted: "#3A4512", faint: "#4E5C1A", flow: "#000000" },
  firn: { floor: "#D1DDEC", ink: "#0E1A2B", muted: "#34465E", faint: "#52647C", flow: "#1C5D99" },
  kablo: { floor: "#2A1A14", ink: "#F3E6DA", muted: "#C4AE9E", faint: "#9C8574", flow: "#E0874A" },
  serit: { floor: "#F8E9CC", ink: "#1E1610", muted: "#574634", faint: "#7A6650", flow: "#1E1610" },
};
const lin = (c) => { c /= 255; return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4; };
const lum = (h) => { const n = parseInt(h.slice(1), 16); return 0.2126 * lin(n >> 16) + 0.7152 * lin((n >> 8) & 255) + 0.0722 * lin(n & 255); };
const ratio = (a, b) => { const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p); return (x + 0.05) / (y + 0.05); };
const need = { ink: 7, muted: 4.5, faint: 3, flow: 3 };
for (const [name, t] of Object.entries(themes)) {
  const row = Object.keys(need).map((k) => {
    const r = ratio(t[k], t.floor);
    return `${k} ${r.toFixed(2)}${r >= need[k] ? "" : " FAIL"}`;
  });
  console.log(name.padEnd(6), row.join("  "));
}
