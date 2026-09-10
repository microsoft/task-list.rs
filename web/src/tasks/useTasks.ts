import { useCallback, useEffect, useState } from 'react';

import { api, type TaskDto } from '../api/client';

/** Discriminated task-list state consumed by the UI. Empty is `ready` with `tasks: []`. */
export type TasksState =
  | { status: 'loading' }
  | { status: 'ready'; tasks: TaskDto[] }
  | { status: 'error'; error: string };

export interface UseTasks {
  state: TasksState;
  /** Create a task and append the server-stamped result. Rejects on failure. */
  createTask: (title: string) => Promise<void>;
}

function messageFrom(error: unknown): string {
  return error instanceof Error ? error.message : 'Something went wrong.';
}

/**
 * Load the owner's tasks once on mount, and expose a `createTask` that appends the
 * server-returned {@link TaskDto} on success (append-on-success — no full refetch, since
 * the 201 body is fully server-stamped). A create failure rejects so the form can surface
 * it; the existing list is left untouched.
 */
export function useTasks(): UseTasks {
  const [state, setState] = useState<TasksState>({ status: 'loading' });

  useEffect(() => {
    let active = true;

    api
      .listTasks()
      .then((tasks) => {
        if (!active) {
          return;
        }
        setState((prev) => {
          // If a `createTask` already advanced us to `ready` while this initial load was
          // in flight, the load is stale: merge its tasks *underneath* the ones already
          // shown (deduped by id) so the just-created task is never dropped. Otherwise the
          // load is authoritative.
          if (prev.status !== 'ready') {
            return { status: 'ready', tasks };
          }
          const seen = new Set(prev.tasks.map((task) => task.id));
          return { status: 'ready', tasks: [...tasks.filter((task) => !seen.has(task.id)), ...prev.tasks] };
        });
      })
      .catch((error: unknown) => {
        if (!active) {
          return;
        }
        // Never mask a task a create already surfaced with a stale initial-load error.
        setState((prev) => (prev.status === 'ready' ? prev : { status: 'error', error: messageFrom(error) }));
      });

    return () => {
      active = false;
    };
  }, []);

  const createTask = useCallback(async (title: string) => {
    const created = await api.createTask({ title });
    setState((prev) =>
      prev.status === 'ready'
        ? { status: 'ready', tasks: [...prev.tasks, created] }
        : { status: 'ready', tasks: [created] },
    );
  }, []);

  return { state, createTask };
}
