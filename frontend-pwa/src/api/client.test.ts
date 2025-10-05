/**
 * @file Regression tests for the handcrafted API client wrappers.
 * These tests ensure the generated OpenAPI client continues to expose the
 * query key helpers and delegates requests through the shared fetcher.
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { UserIdSchema, UsersSchema } from '@app/types';

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
    const userId = UserIdSchema.parse('00000000-0000-0000-0000-000000000000');

    expect(usersQueryKeys.byId(userId)).toEqual(['users', userId]);
    expect(Object.isFrozen(usersQueryKeys)).toBe(true);
  });

  it('delegates listUsers to the shared fetcher with schema validation', async () => {
    const { listUsers } = await import('./client');
    const controller = new AbortController();
    const expected = [
      {
        id: UserIdSchema.parse('11111111-2222-3333-4444-555555555555'),
        displayName: 'Test User',
      },
    ];

    mockCustomFetchParsed.mockResolvedValueOnce(expected);

    const result = await listUsers(controller.signal);

    expect(mockCustomFetchParsed).toHaveBeenCalledWith('/api/users', UsersSchema, {
      signal: controller.signal,
    });
    expect(result).toBe(expected);
  });
});
