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
  assert.throws(() => resolveToken('{a}', tokens), /Circular token reference/);
});

test('throws on missing path with enriched message', () => {
  assert.throws(
    () => resolveToken('{color.missing}', baseTokens),
    /Token path "color.missing" not found \(while resolving "color.missing"\).*Available keys: brand, base, linked/
  );
});

test('throws on invalid tokens arg', () => {
  assert.throws(
    () => resolveToken('{color.brand}', null),
    /tokens must be an object token tree/
  );
});

test('throws on non-string ref', () => {
  assert.throws(
    () => resolveToken(123, baseTokens),
    /ref must be a string like "\{path\.to\.token\}" or a literal string/
  );
});

test('returns the literal when input is a non-braced string', () => {
  assert.equal(resolveToken('plain', baseTokens), 'plain');
});

test('throws when token leaf lacks a string value', () => {
  const tokens = { a: { value: 1 } };
  assert.throws(
    () => resolveToken('{a}', tokens),
    /must resolve to an object with a string "value"/
  );
});
