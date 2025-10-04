/**
 * @file Regression tests for the handcrafted API client wrappers.
 * These tests ensure the generated OpenAPI client continues to expose the
 * query key helpers and delegates requests through the shared fetcher.
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { UsersSchema } from '@app/types';

const mockCustomFetchParsed = vi.fn();

vi.mock('./fetcher', () => ({
  customFetchParsed: mockCustomFetchParsed,
}));

describe('client wrappers', () => {
  afterEach(() => {
    mockCustomFetchParsed.mockReset();
  });

  it('exposes stable query keys for user listings', async () => {
    const { usersQueryKey, usersQueryKeys } = await import('./client');

    expect(usersQueryKey).toEqual(['users']);
    expect(Object.isFrozen(usersQueryKey)).toBe(true);
    expect(usersQueryKeys.all).toBe(usersQueryKey);
    expect(usersQueryKeys.byId('abc')).toEqual(['users', 'abc']);
    expect(Object.isFrozen(usersQueryKeys)).toBe(true);
  });

  it('delegates listUsers to the shared fetcher with schema validation', async () => {
    const { listUsers } = await import('./client');
    const controller = new AbortController();
    const expected = [{ id: 'u-1', displayName: 'Test User' }];

    mockCustomFetchParsed.mockResolvedValueOnce(expected);

    const result = await listUsers(controller.signal);

    expect(mockCustomFetchParsed).toHaveBeenCalledWith('/api/users', UsersSchema, {
      signal: controller.signal,
    });
    expect(result).toBe(expected);
  });
});
