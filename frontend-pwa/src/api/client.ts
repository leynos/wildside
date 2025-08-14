/**
 * @file API client functions generated from OpenAPI.
 */
import { z } from 'zod';
import { customFetchParsed } from './fetcher';

const userSchema = z.object({
  id: z.string(),
  display_name: z.string(),
});
export type User = z.infer<typeof userSchema>;
const usersSchema = z.array(userSchema);

export const listUsers = ({ signal }: { signal?: AbortSignal } = {}) =>
  customFetchParsed('/api/users', usersSchema, { signal });
