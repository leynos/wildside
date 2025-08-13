import { customFetch } from './fetcher';

export interface User {
  id: string;
  display_name: string;
}

export const getUsers = () => customFetch<User[]>('/api/users');
