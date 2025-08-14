/**
 * @file Fetch helper that augments requests with common defaults.
 */

export const customFetch = async <T>(input: string, init?: RequestInit): Promise<T> => {
  const base = import.meta.env.VITE_API_BASE ?? 'http://localhost:8080';
  const url = new URL(input, base);
  const headers = new Headers(init?.headers as HeadersInit | undefined);
  if (!headers.has('Accept')) headers.set('Accept', 'application/json');
  if (init?.body != null && !headers.has('Content-Type')) {
    headers.set('Content-Type', 'application/json');
  }

  const res = await fetch(url, {
    credentials: 'include',
    ...init,
    headers,
  });

  if (!res.ok) {
    let detail: unknown = undefined;
    const ct = res.headers.get('content-type') ?? '';
    try {
      detail = ct.includes('application/json') ? await res.json() : await res.text();
    } catch {
      /* ignore parse errors */
    }
    throw new Error(`${res.status} ${res.statusText}${detail ? `: ${JSON.stringify(detail)}` : ''}`);
  }

  if (res.status === 204) return undefined as unknown as T;
  const ct = res.headers.get('content-type') ?? '';
  const len = res.headers.get('content-length');
  if (!ct.includes('application/json') || len === '0') {
    return undefined as unknown as T;
  }
  return res.json() as Promise<T>;
};
