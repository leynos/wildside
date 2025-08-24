/** @file Tests for design token resolution utilities. */
import test from 'node:test';
import assert from 'node:assert/strict';
import { resolveToken } from './tokens.js';

// helper tokens for tests
const baseTokens = {
  color: {
    brand: { value: '#fff' },
    base: { value: '#000' },
    linked: { value: '{color.base}' },
  },
};

test('resolves a simple value', () => {
  assert.equal(resolveToken('{color.brand}', baseTokens), '#fff');
});

test('resolves a chained reference', () => {
  assert.equal(resolveToken('{color.linked}', baseTokens), '#000');
});

test('throws on circular reference', () => {
  const tokens = { a: { value: '{b}' }, b: { value: '{a}' } };
  assert.throws(
    () => resolveToken('{a}', tokens),
    /Circular token reference detected: "a"/,
  );
});

test('throws on missing path with enriched message', () => {
  try {
    resolveToken('{color.missing}', baseTokens);
    assert.fail('Expected to throw');
  } catch (err) {
    const msg = String((err && err.message) || err);
    assert.match(msg, /Token path "color.missing" not found/);
    assert.match(msg, /(while resolving "color\.missing")/);
    assert.match(msg, /Available keys:/);
    assert.match(msg, /\bbrand\b/);
    assert.match(msg, /\bbase\b/);
    assert.match(msg, /\blinked\b/);
  }
});

test('throws on invalid tokens arg', () => {
  // Assert class
  assert.throws(() => resolveToken('{color.brand}', null), TypeError);
  // Assert message
  assert.throws(
    () => resolveToken('{color.brand}', null),
    /tokens must be an object token tree/,
  );
});

test('throws on non-string ref', () => {
  assert.throws(
    () => resolveToken(123, baseTokens),
    {
      name: 'TypeError',
      message:
        /ref must be a string like "\{path\.to\.token\}" or a literal string/,
    },
  );
});

test('returns the literal when input is a non-braced string', () => {
  assert.equal(resolveToken('plain', baseTokens), 'plain');
});

test('handles optional whitespace inside braces', () => {
  // If the resolver does not support whitespace, adjust implementation or drop this test.
  assert.equal(resolveToken('{  color.brand  }', baseTokens), '#fff');
});

test('throws when token leaf lacks a string value', () => {
  const tokens = { a: { value: 1 } };
  // Assert class
  assert.throws(() => resolveToken('{a}', tokens), TypeError);
  // Assert message
  assert.throws(
    () => resolveToken('{a}', tokens),
    /must resolve to an object with a string "value"/,
  );
});
