import { afterEach, describe, expect, it, vi } from 'vitest';

import { ApiError, apiFetch } from './http';

describe('apiFetch', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('parses a JSON body on success', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => new Response(JSON.stringify({ status: 'ok', time: 't' }), { status: 200 })),
    );

    const body = await apiFetch<{ status: string; time: string }>('/api/health');

    expect(body).toEqual({ status: 'ok', time: 't' });
  });

  it('throws a typed ApiError with the status on failure', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => new Response('nope', { status: 503 })),
    );

    await expect(apiFetch('/api/health')).rejects.toMatchObject({
      name: 'ApiError',
      status: 503,
    });
    await expect(apiFetch('/api/health')).rejects.toBeInstanceOf(ApiError);
  });

  it('sends Content-Type: application/json when a body is present (avoids 415)', async () => {
    const fetchMock = vi
      .fn<typeof fetch>()
      .mockResolvedValue(new Response(JSON.stringify({ id: '1' }), { status: 201 }));
    vi.stubGlobal('fetch', fetchMock);

    await apiFetch('/api/tasks', { method: 'POST', body: JSON.stringify({ title: 'x' }) });

    const headers = fetchMock.mock.calls[0][1]?.headers as Record<string, string>;
    expect(headers['Content-Type']).toBe('application/json');
    expect(headers.Accept).toBe('application/json');
  });

  it('does not set Content-Type on a bodyless GET', async () => {
    const fetchMock = vi
      .fn<typeof fetch>()
      .mockResolvedValue(new Response(JSON.stringify({}), { status: 200 }));
    vi.stubGlobal('fetch', fetchMock);

    await apiFetch('/api/tasks');

    const headers = fetchMock.mock.calls[0][1]?.headers as Record<string, string>;
    expect(headers['Content-Type']).toBeUndefined();
  });

  it('surfaces the RFC7807 detail from a problem+json error body', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(
        async () =>
          new Response(
            JSON.stringify({ title: 'Bad Request', status: 400, detail: 'title must be between 1 and 200 characters' }),
            { status: 400, headers: { 'Content-Type': 'application/problem+json' } },
          ),
      ),
    );

    await expect(apiFetch('/api/tasks', { method: 'POST', body: '{}' })).rejects.toMatchObject({
      status: 400,
      message: 'title must be between 1 and 200 characters',
    });
  });
});
