export const customFetch = async <T>(input: string, init?: RequestInit): Promise<T> => {
  const base = import.meta.env.VITE_API_BASE ?? 'http://localhost:8080';
  const res = await fetch(new URL(input, base), {
    credentials: 'include',
    headers: { 'Content-Type': 'application/json' },
    ...init
  });
  if (!res.ok) throw new Error(`${res.status} ${res.statusText}`);
  return res.json() as Promise<T>;
};
