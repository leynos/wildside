#!/usr/bin/env node
/** @file Validate theme colour contrast.
 * Ensures brand and accent colours in theme tokens meet WCAG AA contrast
 * against their paired text colour. This guards against inaccessible colour
 * combinations slipping into the design system.
 */
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';
import { readJson } from '../build-utils/read-json.js';
import { contrast } from '../src/utils/color.js';
import { resolveToken } from '../src/utils/tokens.js';

// Load package settings for defaults.
/**
 * @typedef {{name: string, version: string, contrastThreshold?: number}} PackageJson
 */
/** @type {PackageJson} */
const pkgJson = readJson(new URL('../package.json', import.meta.url));
validatePkgJson(pkgJson);

/**
 * Check for required package.json fields.
 *
 * @param {unknown} json - Parsed package.json content.
 * @returns {boolean} True if structure and fields are valid.
 * @example
 * ```js
 * hasRequiredPkgFields({ name: 'pkg', version: '1.0.0' });
 * //=> true
 * hasRequiredPkgFields({});
 * //=> false
 * ```
 */
function hasRequiredPkgFields(json) {
  return (
    json &&
    typeof json === 'object' &&
    typeof json.name === 'string' &&
    typeof json.version === 'string'
  );
}

/**
 * Validate the structure of package.json.
 *
 * @param {unknown} json - Parsed package.json content.
 * @throws {Error} If required fields are missing.
 * @example
 * ```js
 * validatePkgJson({ name: 'pkg', version: '1.0.0' });
 * // passes
 * validatePkgJson({});
 * // throws Error
 * ```
 */
function validatePkgJson(json) {
  if (!hasRequiredPkgFields(json)) {
    throw new Error('package.json must be a valid object with "name" and "version" string fields.');
  }
}

/** Resolve the contrast threshold from CLI or package.json.
 *
 * @returns {number} Desired contrast ratio.
 * @example
 * ```js
 * // CLI: node validate-contrast.js 5 -> returns 5
 * // Or fallback to package.json "contrastThreshold"
 * getThreshold(); //=> 4.5
 * ```
 */
/**
 * Determine whether a threshold falls within WCAG's valid range.
 *
 * @param {number} value - Candidate contrast ratio.
 * @returns {boolean} True if > 1 and < 21.
 * @example
 * ```js
 * isValidThresholdRange(4.5); //=> true
 * isValidThresholdRange(25); //=> false
 * ```
 */
function isValidThresholdRange(value) {
  return value > 1 && value < 21;
}

/**
 * Parse a potential contrast threshold value.
 *
 * @param {unknown} src - Raw threshold source.
 * @returns {number | null} Parsed value or null if invalid.
 */
function parseThresholdSource(src) {
  const value = parseFloat(src);
  if (Number.isNaN(value)) return null;
  if (!isValidThresholdRange(value)) {
    console.error(
      `Error: contrastThreshold value (${value}) is out of range. It must be > 1 and < 21.`,
    );
    process.exit(1);
  }
  return value;
}

function getThreshold() {
  const sources = [process.argv[2], pkgJson.contrastThreshold];
  for (const src of sources) {
    const value = parseThresholdSource(src);
    if (value !== null) return value;
  }
  return 4.5;
}

const contrastThreshold = getThreshold();

/**
 * Validate contrast ratios for brand and accent pairs within a theme file.
 * Returns an array of error messages rather than throwing to allow full
 * aggregation of failures.
 *
 * @param {string | URL} file - Theme file to check.
 * @param {number} threshold - Minimum required contrast.
 * @returns {string[]} Any contrast violations.
 * @example
 * ```js
 * const errs = checkTheme(new URL('src/themes/light.json', import.meta.url), 4.5);
 * if (errs.length) console.error(errs);
 * ```
 */
/**
 * Resolve tokens and compute their contrast ratio.
 *
 * @param {string} fgRef - Foreground token reference.
 * @param {string} bgRef - Background token reference.
 * @returns {number} Contrast ratio.
 * @example
 * ```js
 * calculateColorRatio('{color.fg}', '{color.bg}');
 * //=> 4.5
 * ```
 */
function calculateColorRatio(fgRef, bgRef) {
  const fg = resolveToken(fgRef);
  const bg = resolveToken(bgRef);
  return contrast(fg, bg);
}

/**
 * Validate a foreground/background colour pair against the contrast threshold.
 *
 * @param {{label: string, fgRef?: string, bgRef?: string}} colorPair - Pair to check.
 * @param {{threshold: number, fileHint: string}} context - Threshold and error context.
 * @returns {string | null} Error message if validation fails.
 * @example
 * ```js
 * validateColorPair(
 *   { label: 'brand', fgRef: '{fg}', bgRef: '{bg}' },
 *   { threshold: 4.5, fileHint: 'theme.json' },
 * );
 * //=> null
 * ```
 */
function validateColorPair(colorPair, context) {
  const { label, fgRef, bgRef } = colorPair;
  const { threshold, fileHint } = context;
  const invalidRef =
    typeof fgRef !== 'string' ||
    typeof bgRef !== 'string' ||
    fgRef.trim() === '' ||
    bgRef.trim() === '';
  if (invalidRef) return `${label} in ${fileHint} is missing a value or contrast token`;
  try {
    const ratio = calculateColorRatio(fgRef, bgRef);
    return ratio < threshold
      ? `${label} in ${fileHint} fails contrast: ${ratio.toFixed(2)} (threshold: ${threshold})`
      : null;
  } catch (err) {
    console.error(`Failed to resolve token reference for "${label}" in ${fileHint}.`, {
      fgRef,
      bgRef,
      error: err,
    });
    return `${label} in ${fileHint} failed to resolve token reference: ${
      err instanceof Error ? err.message : String(err)
    }`;
  }
}

function checkTheme(file, threshold) {
  /**
   * @typedef {{name?: string, semantic: {brand?: object, accent?: object}}} ThemeJson
   */
  /** @type {ThemeJson} */
  const json = readJson(file);
  validateThemeJson(json, file);

  const { brand, accent } = json.semantic ?? {};
  const fileHint = file instanceof URL ? file.pathname : file;
  if (!brand || !accent) {
    return [`Missing brand/accent in ${fileHint}`];
  }

  const pairs = [
    ['brand default', brand.default?.value, brand.contrast?.value],
    ['brand hover', brand.hover?.value, brand.contrast?.value],
    ['accent default', accent.default?.value, accent.contrast?.value],
    ['accent hover', accent.hover?.value, accent.contrast?.value],
  ];

  return pairs
    .map(([label, fgRef, bgRef]) =>
      validateColorPair({ label, fgRef, bgRef }, { threshold, fileHint }),
    )
    .filter((error) => error !== null);
}

/**
 * Ensure loaded JSON is a plain object.
 *
 * @param {unknown} json - Parsed JSON to verify.
 * @param {string} fileHint - File path for error context.
 * @throws {Error} If the JSON is not an object.
 * @example
 * ```js
 * validateJsonStructure({}, 'theme.json');
 * // passes
 * validateJsonStructure(null, 'theme.json');
 * // throws Error
 * ```
 */
function validateJsonStructure(json, fileHint) {
  if (!json || typeof json !== 'object') {
    throw new Error(`Theme file ${fileHint} is not a valid JSON object.`);
  }
}

/**
 * Ensure the "semantic" object exists.
 *
 * @param {{semantic?: unknown}} json - Parsed theme JSON.
 * @param {string} fileHint - File path for error context.
 * @throws {Error} If "semantic" is missing or not an object.
 * @example
 * ```js
 * validateSemanticObject({ semantic: {} }, 'theme.json');
 * // passes
 * validateSemanticObject({}, 'theme.json');
 * // throws Error
 * ```
 */
function validateSemanticObject(json, fileHint) {
  if (!json.semantic || typeof json.semantic !== 'object') {
    throw new Error(`Theme file ${fileHint} is missing "semantic" object.`);
  }
}

/**
 * Ensure the "semantic" object includes brand and accent fields.
 *
 * @param {{brand?: unknown, accent?: unknown}} semantic - Semantic section.
 * @param {string} fileHint - File path for error context.
 * @throws {Error} If brand or accent are missing.
 * @example
 * ```js
 * validateBrandAndAccent({ brand: {}, accent: {} }, 'theme.json');
 * // passes
 * validateBrandAndAccent({}, 'theme.json');
 * // throws Error
 * ```
 */
function validateBrandAndAccent(semantic, fileHint) {
  if (!semantic.brand || !semantic.accent) {
    throw new Error(
      `Theme file ${fileHint} must contain "semantic.brand" and "semantic.accent" fields.`,
    );
  }
}

/**
 * Validate the structure of a theme JSON file.
 *
 * @param {unknown} json - Parsed theme content.
 * @param {string | URL} file - File path for error context.
 * @throws {Error} If required fields are missing.
 * @example
 * ```js
 * validateThemeJson({ semantic: { brand: {}, accent: {} } }, 'theme.json');
 * // passes
 * validateThemeJson({}, 'theme.json');
 * // throws Error
 * ```
 */
function validateThemeJson(json, file) {
  const fileHint = file instanceof URL ? file.pathname : file;
  validateJsonStructure(json, fileHint);
  validateSemanticObject(json, fileHint);
  // At this point json.semantic is a valid object.
  validateBrandAndAccent(json.semantic, fileHint);
}

const themesUrl = new URL('../src/themes/', import.meta.url);
// Convert the URL to a file-system path for cross-platform compatibility.
const themeFiles = fs
  .readdirSync(fileURLToPath(themesUrl))
  .filter((f) => f.endsWith('.json'))
  .map((f) => new URL(f, themesUrl));

let allErrors = [];
for (const file of themeFiles) {
  allErrors = allErrors.concat(checkTheme(file, contrastThreshold));
}

if (allErrors.length) {
  for (const err of allErrors) {
    console.error(err);
  }
  process.exit(1);
}

console.log(`Contrast checks passed for themes (threshold: ${contrastThreshold}).`);
process.exit(0);
