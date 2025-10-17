/**
 * @file Test helpers for working with the Vite logger interface.
 */

import type { Logger } from 'vite';
import { vi } from 'vitest';

type LoggerExtensions = {
  time: ReturnType<typeof vi.fn>;
  timeEnd: ReturnType<typeof vi.fn>;
  debug: ReturnType<typeof vi.fn>;
  fatal: ReturnType<typeof vi.fn>;
};

export function createMockLogger(): Logger {
  let errorLogged = false;
  const info = vi.fn();
  const warn = vi.fn();
  const warnOnce = vi.fn();
  const error = vi.fn(() => {
    errorLogged = true;
  });
  const clearScreen = vi.fn();
  const time = vi.fn();
  const timeEnd = vi.fn();
  const debug = vi.fn();
  const fatal = vi.fn(() => {
    errorLogged = true;
  });
  const hasErrorLogged = vi.fn(() => errorLogged);

  const logger: Logger & LoggerExtensions = {
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
    hasErrorLogged,
  };

  return logger;
}
