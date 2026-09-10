import type { components } from '../ApiClient.generated';
import { apiFetch } from './http';

// Types flow from the OpenAPI doc via `openapi-typescript` — never hand-written.
export type HealthResponse = components['schemas']['HealthResponse'];
export type TaskDto = components['schemas']['TaskDto'];
export type TaskStatusDto = components['schemas']['TaskStatusDto'];
export type CreateTaskRequest = components['schemas']['CreateTaskRequest'];

/** Typed façade over the API surface, built on the generated client types. */
export const api = {
  getHealth: () => apiFetch<HealthResponse>('/api/health'),
  listTasks: () => apiFetch<TaskDto[]>('/api/tasks'),
  // POST with a JSON body: `apiFetch` sets `Content-Type: application/json` (else 415).
  createTask: (req: CreateTaskRequest) =>
    apiFetch<TaskDto>('/api/tasks', { method: 'POST', body: JSON.stringify(req) }),
};
