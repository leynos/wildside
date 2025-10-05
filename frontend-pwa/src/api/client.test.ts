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

    const userKey = usersQueryKeys.byId(userId);

    expect(userKey).toEqual(['users', userId]);
    expect(Object.isFrozen(userKey)).toBe(true);
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

  it('propagates schema validation failures from the fetcher', async () => {
    const { listUsers } = await import('./client');
    const validationError = new Error('Invalid user schema');

    mockCustomFetchParsed.mockRejectedValueOnce(validationError);

    await expect(listUsers()).rejects.toThrow('Invalid user schema');
  });

  it('surfaces network errors to the caller', async () => {
    const { listUsers } = await import('./client');
    const networkError = new Error('Network failure');

    mockCustomFetchParsed.mockRejectedValueOnce(networkError);

    await expect(listUsers()).rejects.toThrow('Network failure');
  });

  it('propagates abort errors when the request is cancelled', async () => {
    const { listUsers } = await import('./client');
    const controller = new AbortController();

    controller.abort();

    const abortError = new DOMException('AbortError', 'AbortError');

    mockCustomFetchParsed.mockRejectedValueOnce(abortError);

    await expect(listUsers(controller.signal)).rejects.toThrow('AbortError');
  });
});
