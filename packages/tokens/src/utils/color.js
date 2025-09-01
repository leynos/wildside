/** @file Colour utilities built on the `color` library.
 * Provides helpers for calculating WCAG contrast ratios.
 */
import Color from 'color';

/**
 * Create a descriptive TypeError for invalid colour inputs, preserving cause.
 *
 * @param {string} name - Human-friendly name of the colour argument.
 * @param {unknown} value - Original value passed by the caller.
 * @param {unknown} cause - Underlying error thrown by the colour parser.
 * @returns {TypeError} Error instance with stable message and attached cause.
 */
function createInvalidColourError(name, value, cause) {
  return new TypeError(`Invalid ${name}: ${String(value)}`, { cause });
}

function parseColour(value, name = 'colour') {
  try {
    return Color(value);
  } catch (err) {
    // Preserve underlying error details for debugging across runtimes
    throw createInvalidColourError(name, value, err);
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
