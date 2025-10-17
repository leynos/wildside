/**
 * @file Test helpers for working with the Vite logger interface.
 */

import type { Logger } from 'vite';
import { vi } from 'vitest';

export function createMockLogger(): Logger {
  let errorLogged = false;
  const info = vi.fn();
  const warn = vi.fn();
  const warnOnce = vi.fn();
  const error = vi.fn(() => {
    errorLogged = true;
  });
  const clearScreen = vi.fn();
  const hasErrorLogged = vi.fn(() => errorLogged);

  const logger: Logger = {
    hasWarned: false,
    info,
    warn,
    warnOnce,
    error,
    clearScreen,
    hasErrorLogged,
  };

  return logger;
}
