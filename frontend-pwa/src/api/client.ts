/**
 * @file API client functions generated from OpenAPI.
 * Uses shared user types from `@app/types`.
 */
import { customFetchParsed } from './fetcher';
import { User, UserSchema } from '@app/types';

/**
 * Fetch all registered users.
 *
 * @example
 * const users = await listUsers();
 * users.length;
 */
export const listUsers = (
  signal?: AbortSignal,
): Promise<User[]> =>
  customFetchParsed('/api/users', UserSchema.array(), { signal });
