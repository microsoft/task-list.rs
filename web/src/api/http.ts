/** Error thrown by {@link apiFetch} when the API responds with a non-2xx status. */
export class ApiError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

/**
 * Base fetch wrapper for same-origin `/api/*` calls. Sends/accepts JSON, and turns non-2xx
 * responses into a typed {@link ApiError}. The generated client types (ApiClient.generated)
 * describe the shapes; this wrapper carries them over the wire.
 *
 * A request that carries a body sends `Content-Type: application/json` — the backend's JSON
 * extractor rejects a bodied request without it as 415 (see api/tests/http.rs).
 */
export async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const headers: Record<string, string> = {
    Accept: 'application/json',
    ...(init?.body != null ? { 'Content-Type': 'application/json' } : {}),
    ...(init?.headers as Record<string, string> | undefined),
  };

  const response = await fetch(path, { ...init, headers });

  if (!response.ok) {
    throw new ApiError(response.status, await errorMessage(response, init?.method ?? 'GET', path));
  }

  return (await response.json()) as T;
}

/**
 * Build a human-readable error message, preferring the RFC7807 `detail`/`title` when the
 * server sent an `application/problem+json` document (e.g. a 400 validation message).
 */
async function errorMessage(response: Response, method: string, path: string): Promise<string> {
  try {
    const problem = (await response.json()) as { detail?: unknown; title?: unknown };
    const detail = typeof problem.detail === 'string' ? problem.detail : undefined;
    const title = typeof problem.title === 'string' ? problem.title : undefined;
    if (detail ?? title) {
      return detail ?? title!;
    }
  } catch {
    // Non-JSON body — fall through to a generic message.
  }
  return `${method} ${path} failed (${response.status})`;
}
