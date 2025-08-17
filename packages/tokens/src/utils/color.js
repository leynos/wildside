/** @file Colour utilities built on the `color` library.
 * Provides helpers for calculating WCAG contrast ratios.
 */
import Color from 'color';

/**
 * Compute the WCAG contrast ratio between two colours.
 *
 * @param {string} foreground - Foreground colour in hex format.
 * @param {string} background - Background colour in hex format.
 * @returns {number} Contrast ratio.
 * @example
 * contrast('#000', '#fff'); // => 21
 */
export function contrast(foreground, background) {
  const fg = Color(foreground);
  const bg = Color(background);
  return fg.contrast(bg);
}
