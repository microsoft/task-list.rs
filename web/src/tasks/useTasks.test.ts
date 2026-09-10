import { renderHook, waitFor, act } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { api, type TaskDto } from '../api/client';
import { useTasks } from './useTasks';

vi.mock('../api/client', () => ({
  api: {
    listTasks: vi.fn(),
    createTask: vi.fn(),
  },
}));

const listTasks = vi.mocked(api.listTasks);
const createTask = vi.mocked(api.createTask);

/** A promise whose settlement the test drives explicitly — deterministic ordering. */
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function task(id: string, title: string): TaskDto {
  return {
    id,
    owner: 'demo|user',
    title,
    status: 'todo',
    createdAt: '2026-07-09T00:00:00Z',
    updatedAt: '2026-07-09T00:00:00Z',
  };
}

describe('useTasks', () => {
  afterEach(() => {
    vi.clearAllMocks();
  });

  it('loads the owner tasks into a ready state', async () => {
    listTasks.mockResolvedValue([task('1', 'seed')]);

    const { result } = renderHook(() => useTasks());

    await waitFor(() => expect(result.current.state.status).toBe('ready'));
    expect(result.current.state).toEqual({ status: 'ready', tasks: [task('1', 'seed')] });
  });

  it('surfaces a load failure as an error state', async () => {
    listTasks.mockRejectedValue(new Error('offline'));

    const { result } = renderHook(() => useTasks());

    await waitFor(() => expect(result.current.state.status).toBe('error'));
    expect(result.current.state).toEqual({ status: 'error', error: 'offline' });
  });

  it('appends the server-stamped task returned by create (no refetch)', async () => {
    listTasks.mockResolvedValue([task('1', 'seed')]);
    createTask.mockResolvedValue(task('2', 'new'));

    const { result } = renderHook(() => useTasks());
    await waitFor(() => expect(result.current.state.status).toBe('ready'));

    await act(async () => {
      await result.current.createTask('new');
    });

    expect(createTask).toHaveBeenCalledWith({ title: 'new' });
    expect(listTasks).toHaveBeenCalledTimes(1); // no refetch after create
    expect(result.current.state).toEqual({
      status: 'ready',
      tasks: [task('1', 'seed'), task('2', 'new')],
    });
  });

  it('rejects and leaves the list untouched when create fails', async () => {
    listTasks.mockResolvedValue([task('1', 'seed')]);
    createTask.mockRejectedValue(new Error('bad request'));

    const { result } = renderHook(() => useTasks());
    await waitFor(() => expect(result.current.state.status).toBe('ready'));

    await expect(
      act(async () => {
        await result.current.createTask('new');
      }),
    ).rejects.toThrow('bad request');

    expect(result.current.state).toEqual({ status: 'ready', tasks: [task('1', 'seed')] });
  });

  it('keeps a task created during the initial load after the stale load settles', async () => {
    // The initial load stays in flight (cold start) until the test resolves it.
    const load = deferred<TaskDto[]>();
    listTasks.mockReturnValue(load.promise);
    createTask.mockResolvedValue(task('2', 'new'));

    const { result } = renderHook(() => useTasks());
    expect(result.current.state.status).toBe('loading');

    // A create lands BEFORE the initial list load resolves.
    await act(async () => {
      await result.current.createTask('new');
    });
    expect(result.current.state).toEqual({ status: 'ready', tasks: [task('2', 'new')] });

    // Now the stale initial load settles — it must NOT drop the created task.
    await act(async () => {
      load.resolve([task('1', 'seed')]);
      await load.promise;
    });

    // The load's tasks are merged underneath; the created task remains.
    expect(result.current.state).toEqual({
      status: 'ready',
      tasks: [task('1', 'seed'), task('2', 'new')],
    });
  });

  it('does not mask a task created during the initial load with a stale load error', async () => {
    const load = deferred<TaskDto[]>();
    listTasks.mockReturnValue(load.promise);
    createTask.mockResolvedValue(task('2', 'new'));

    const { result } = renderHook(() => useTasks());

    await act(async () => {
      await result.current.createTask('new');
    });
    expect(result.current.state).toEqual({ status: 'ready', tasks: [task('2', 'new')] });

    // The stale initial load fails after the create — the created task must survive.
    await act(async () => {
      load.reject(new Error('offline'));
      await load.promise.catch(() => undefined);
    });

    expect(result.current.state).toEqual({ status: 'ready', tasks: [task('2', 'new')] });
  });
});
