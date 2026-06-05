/**
 * @file Test helpers for working with the Vite logger interface.
 */

import { mock } from 'bun:test';
import type { Logger } from 'vite';

type MockFunction = ReturnType<typeof mock>;

type LoggerExtensions = {
  time: MockFunction;
  timeEnd: MockFunction;
  debug: MockFunction;
  fatal: MockFunction;
};

export function createMockLogger(): Logger {
  let errorLogged = false;
  const info = mock();
  const warn = mock();
  const warnOnce = mock();
  const error = mock(() => {
    errorLogged = true;
  });
  const clearScreen = mock();
  const time = mock();
  const timeEnd = mock();
  const debug = mock();
  const fatal = mock(() => {
    errorLogged = true;
  });
  const hasErrorLogged = mock(() => errorLogged);

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
