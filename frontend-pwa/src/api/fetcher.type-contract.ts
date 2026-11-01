/**
 * @file Compile-time contract checks for the shared fetcher.
 *
 * `make typecheck` compiles this module, so each `@ts-expect-error` below is
 * an assertion that the marked call is rejected. Nothing here runs at runtime.
 */

import { z } from 'zod';

import { customFetch, customFetchOrval, customFetchParsed, customFetchParsedSafe } from './fetcher';

const userSchema = z.object({ id: z.string() });

/**
 * Type-level assertions for the fetcher's accepted and rejected usage.
 *
 * The function is never called; `tsc --noEmit` is the test.
 */
export async function fetcherTypeContracts(): Promise<void> {
  // Accepted JSON bodies: objects, arrays, and scalars.
  await customFetch('/api/example', { method: 'POST', body: { id: 'user-1' } });
  await customFetch('/api/example', { method: 'POST', body: [1, 'two'] });
  await customFetch('/api/example', { method: 'POST', body: 5 });
  await customFetch('/api/example', { method: 'POST', body: true });

  // Accepted native bodies keep their `BodyInit` identity.
  await customFetch('/api/example', { method: 'POST', body: new FormData() });
  await customFetch('/api/example', { method: 'POST', body: new Blob(['data']) });
  await customFetch('/api/example', { method: 'POST', body: new URLSearchParams() });
  await customFetch('/api/example', { method: 'POST', body: new ArrayBuffer(1) });
  await customFetch('/api/example', { method: 'POST', body: new Uint8Array([1]) });

  // Orval's generated clients pass the response type as a type argument, so the
  // mutator adapter must accept one and surface it to the caller.
  const orvalUsers = await customFetchOrval<Array<{ id: string }>>('/api/users');
  const orvalUserId: string = orvalUsers[0]?.id ?? '';
  orvalUserId.trim();

  // Schema output is inferred for the parsed helpers.
  const user = await customFetchParsed('/api/user', userSchema);
  const userId: string = user.id;
  userId.trim();

  const safe = await customFetchParsedSafe('/api/user', userSchema);
  if (safe.success) {
    const safeId: string = safe.data.id;
    safeId.trim();
  }

  // The response is a JSON value, not text or an arbitrary class instance.
  // @ts-expect-error - narrow the JSON value before treating it as text.
  const text: string = await customFetch('/api/example');
  text.trim();

  // @ts-expect-error - a `Map` is not a fetch body type.
  await customFetch('/api/example', { method: 'POST', body: new Map() });

  // @ts-expect-error - request options are not free-form.
  await customFetch('/api/example', { method: 'POST', bodyy: 'nope' });

  // @ts-expect-error - the parsed helper requires a Zod schema.
  await customFetchParsed('/api/user', 'not a schema');

  // @ts-expect-error - schema output types the result, not the input.
  const wrongId: number = user.id;
  wrongId.toFixed(0);
}
