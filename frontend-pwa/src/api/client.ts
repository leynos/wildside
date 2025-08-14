/**
 * @file API client functions generated from OpenAPI.
 * Uses shared user types from `@app/types`.
 */
import { customFetch } from './fetcher';
import { User, UserSchema } from '@app/types';

/** Fetch all registered users. */
export const listUsers = async (
  signal?: AbortSignal,
): Promise<User[]> => {
  const data = await customFetch<unknown>('/api/users', { signal });
  return UserSchema.array().parse(data);
};
