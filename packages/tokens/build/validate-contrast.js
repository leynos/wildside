/** @file Validate theme colour contrast.
 * Ensures brand and accent colours in theme tokens meet WCAG AA contrast
 * against their paired text colour. This guards against inaccessible colour
 * combinations slipping into the design system.
 */
import fs from 'node:fs';

/** Flatten nested token objects to dot-notated paths. */
function flattenTokens(obj, path = [], out = {}) {
  for (const [key, value] of Object.entries(obj)) {
    const nextPath = [...path, key];
    if (value && typeof value === 'object' && 'value' in value) {
      out[nextPath.join('.')] = value.value;
    } else if (value && typeof value === 'object') {
      flattenTokens(value, nextPath, out);
    }
  }
  return out;
}

/** Resolve `{token.path}` references to concrete hex values. */
function resolve(val, tokens) {
  const match = /^\{(.+)\}$/.exec(val);
  return match ? tokens[match[1]] : val;
}

/** Calculate relative luminance for a hex colour. */
function luminance(hex) {
  const [r, g, b] = hex
    .replace('#', '')
    .match(/.{2}/g)
    .map(c => parseInt(c, 16) / 255)
    .map(c => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** Compute WCAG contrast ratio between two hex colours. */
function contrast(a, b) {
  const l1 = luminance(a);
  const l2 = luminance(b);
  const [lighter, darker] = l1 > l2 ? [l1, l2] : [l2, l1];
  return (lighter + 0.05) / (darker + 0.05);
}

function checkTheme(file, tokens) {
  const json = JSON.parse(fs.readFileSync(file, 'utf8'));
  const brand = json.semantic?.brand;
  const accent = json.semantic?.accent;
  if (!brand || !accent) throw new Error(`Missing brand/accent in ${file}`);
  const pairs = [
    ['brand default', brand.default?.value, brand.contrast?.value],
    ['brand hover', brand.hover?.value, brand.contrast?.value],
    ['accent default', accent.default?.value, accent.contrast?.value],
    ['accent hover', accent.hover?.value, accent.contrast?.value]
  ];
  for (const [label, fg, bg] of pairs) {
    const fgHex = resolve(fg, tokens);
    const bgHex = resolve(bg, tokens);
    const ratio = contrast(fgHex, bgHex);
    if (ratio < 4.5) {
      throw new Error(`${label} in ${file} fails contrast: ${ratio.toFixed(2)}`);
    }
  }
}

const tokens = flattenTokens(
  JSON.parse(fs.readFileSync(new URL('../src/tokens.json', import.meta.url)))
);

for (const theme of ['dark', 'light']) {
  checkTheme(new URL(`../src/themes/${theme}.json`, import.meta.url), tokens);
}

console.log('Contrast checks passed for themes.');
