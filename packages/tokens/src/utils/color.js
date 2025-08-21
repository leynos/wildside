/** @file Colour utilities built on the `color` library.
 * Provides helpers for calculating WCAG contrast ratios.
 */
import Color from 'color';

function parseColour(value, name = 'colour') {
  try {
    return Color(value);
  } catch (err) {
    const message = `Invalid ${name}: ${String(value)}`;
    // Preserve underlying error details for debugging across runtimes
    throw new TypeError(message, { cause: err });
  }
}

/**
 * Compute the WCAG contrast ratio between two colours.
 *
 * @param {string} foreground - CSS colour string (e.g., hex, rgb, hsl).
 * @param {string} background - CSS colour string (e.g., hex, rgb, hsl).
 * @returns {number} Contrast ratio.
 * @example
 * contrast('#000', '#fff'); // => 21
 */
export function contrast(foreground, background) {
  const fg = parseColour(foreground, 'foreground colour');
  const bg = parseColour(background, 'background colour');
  return fg.contrast(bg);
}
