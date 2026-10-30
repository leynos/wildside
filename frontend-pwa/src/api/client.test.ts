/**
 * @file Regression tests for the handcrafted API client wrappers.
 * These tests ensure the generated OpenAPI client continues to expose the
 * query key helpers and delegates requests through the shared fetcher.
 */

import { afterEach, describe, expect, it, mock, spyOn } from 'bun:test';
import { UserIdSchema, type Users, UsersSchema } from '@app/types';
import { ZodError } from 'zod';

/** A single wire-shaped user record before schema validation. */
interface WireUser {
  id: string;
  // biome-ignore lint/style/useNamingConvention: serialized wire payload key.
  display_name: string | number;
}

// The runtime schema validates the wire payload's snake_case `display_name`
// even though the generated TypeScript type names the field `displayName`.
// Parsing the fixtures through the published schema keeps these tests on the
// same wire contract the client actually consumes.
const users = UsersSchema.parse([
  {
    id: '11111111-2222-3333-4444-555555555555',
    // biome-ignore lint/style/useNamingConvention: serialized wire payload key.
    display_name: 'Test User',
  },
]);

const invalidUsers: WireUser[] = [
  {
    id: 'not-a-uuid',
    // biome-ignore lint/style/useNamingConvention: serialized wire payload key.
    display_name: 42,
  },
];

/** Build a JSON response so the real fetcher can decode it. */
const jsonResponse = (body: Users | WireUser[]): Response => Response.json(body);

describe('client wrappers', () => {
  afterEach(() => {
    mock.restore();
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
    const fetchSpy = spyOn(globalThis, 'fetch').mockResolvedValue(jsonResponse(users));

    const result = await listUsers(controller.signal);

    expect(result).toEqual(users);
    expect(fetchSpy).toHaveBeenCalledTimes(1);
    const [requestUrl, requestInit] = fetchSpy.mock.calls[0];
    expect(new URL(String(requestUrl)).pathname).toBe('/api/users');
    expect(requestInit?.signal).toBe(controller.signal);
    expect(new Headers(requestInit?.headers).get('Accept')).toBe('application/json');
  });

  it('propagates schema validation failures from the fetcher', async () => {
    const { listUsers } = await import('./client');
    spyOn(globalThis, 'fetch').mockResolvedValue(jsonResponse(invalidUsers));

    await expect(listUsers()).rejects.toBeInstanceOf(ZodError);
  });

  it('surfaces network errors to the caller', async () => {
    const { listUsers } = await import('./client');
    spyOn(globalThis, 'fetch').mockRejectedValue(new Error('Network failure'));

    await expect(listUsers()).rejects.toThrow('Network failure');
  });

  it('propagates abort errors when the request is cancelled', async () => {
    const { listUsers } = await import('./client');
    const controller = new AbortController();

    controller.abort();

    spyOn(globalThis, 'fetch').mockRejectedValue(
      new DOMException('The operation was aborted.', 'AbortError'),
    );

    await expect(listUsers(controller.signal)).rejects.toThrow('aborted');
  });
});
