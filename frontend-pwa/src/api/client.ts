/**
 * @file API client functions generated from OpenAPI.
 */
import { customFetch } from './fetcher';

export interface User {
  id: string;
  display_name: string;
}

export const listUsers = () => customFetch<User[]>('/api/users');
