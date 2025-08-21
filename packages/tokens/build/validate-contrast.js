#!/usr/bin/env node
/** @file Validate theme colour contrast.
 * Ensures brand and accent colours in theme tokens meet WCAG AA contrast
 * against their paired text colour. This guards against inaccessible colour
 * combinations slipping into the design system.
 */
import fs from 'node:fs';
import { contrast } from '../src/utils/color.js';
import { resolveToken } from '../src/utils/tokens.js';

// Load package settings for defaults.
const pkgJson = JSON.parse(fs.readFileSync(new URL('../package.json', import.meta.url), 'utf8'));

/** Resolve the contrast threshold from CLI, env, or package.json. */
function getThreshold() {
  const sources = [process.argv[2], process.env.CONTRAST_THRESHOLD, pkgJson.contrastThreshold];
  for (const src of sources) {
    const value = parseFloat(src);
    if (!Number.isNaN(value)) {
      if (value <= 1 || value >= 21) {
        console.error(
          `Error: contrastThreshold value (${value}) is out of range. It must be > 1 and < 21.`,
        );
        process.exit(1);
      }
      return value;
    }
  }
  return 4.5;
}

const contrastThreshold = getThreshold();

/**
 * Validate contrast ratios for brand and accent pairs within a theme file.
 * Returns an array of error messages rather than throwing to allow full
 * aggregation of failures.
 */
function checkTheme(file, threshold) {
  const json = JSON.parse(fs.readFileSync(file, 'utf8'));
  const brand = json.semantic?.brand;
  const accent = json.semantic?.accent;
  const errors = [];

  if (!brand || !accent) {
    errors.push(`Missing brand/accent in ${file instanceof URL ? file.pathname : file}`);
    return errors;
  }

  const pairs = [
    ['brand default', brand.default?.value, brand.contrast?.value],
    ['brand hover', brand.hover?.value, brand.contrast?.value],
    ['accent default', accent.default?.value, accent.contrast?.value],
    ['accent hover', accent.hover?.value, accent.contrast?.value],
  ];

  for (const [label, fgRef, bgRef] of pairs) {
    const fileHint = file instanceof URL ? file.pathname : file;
    if (fgRef == null || bgRef == null) {
      errors.push(`${label} in ${fileHint} is missing a value or contrast token`);
      continue;
    }
    try {
      const ratio = contrast(resolveToken(fgRef), resolveToken(bgRef));
      if (ratio < threshold) {
        errors.push(
          `${label} in ${fileHint} fails contrast: ${ratio.toFixed(2)} (threshold: ${threshold})`,
        );
      }
    } catch (err) {
      console.error(`Failed to resolve token reference for "${label}" in ${fileHint}.`, {
        fgRef,
        bgRef,
        error: err,
      });
      errors.push(
        `${label} in ${fileHint} failed to resolve token reference: ${
          err instanceof Error ? err.message : String(err)
        }`,
      );
    }
  }

  return errors;
}

const themesDir = new URL('../src/themes/', import.meta.url);
const themeFiles = fs
  .readdirSync(themesDir)
  .filter((f) => f.endsWith('.json'))
  .map((f) => new URL(f, themesDir));

let allErrors = [];
for (const file of themeFiles) {
  allErrors = allErrors.concat(checkTheme(file, contrastThreshold));
}

if (allErrors.length) {
  allErrors.forEach((e) => console.error(e));
  process.exit(1);
}

console.log(`Contrast checks passed for themes (threshold: ${contrastThreshold}).`);
process.exit(0);
