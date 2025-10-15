/**
 * @file Test helpers for working with the Vite logger interface.
 */
import { vi } from 'vitest';
import type { Logger } from 'vite';

export function createMockLogger(): Logger {
  const info = vi.fn();
  const warn = vi.fn();
  const warnOnce = vi.fn();
  const error = vi.fn();
  const clearScreen = vi.fn();
  const time = vi.fn();
  const timeEnd = vi.fn();
  const debug = vi.fn();
  const fatal = vi.fn();

  return {
    hasWarned: false,
    info,
    warn,
    warnOnce,
    error,
    clearScreen,
    time,
    timeEnd,
    debug,
    fatal,
  } as unknown as Logger;
}
