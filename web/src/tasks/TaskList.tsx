import type { TaskStatusDto } from '../api/client';
import type { TasksState } from './useTasks';

const STATUS_LABELS: Record<TaskStatusDto, string> = {
  todo: 'To do',
  inProgress: 'In progress',
  done: 'Done',
};

/**
 * Presentational task list with explicit loading / error / empty / populated states.
 * Prop-driven (state passed in) so it is trivially testable without mocking fetch.
 */
export function TaskList({ state }: { state: TasksState }) {
  if (state.status === 'loading') {
    return (
      <p className="tasks__status" role="status">
        Loading tasks…
      </p>
    );
  }

  if (state.status === 'error') {
    return (
      <p className="tasks__status tasks__status--error" role="alert">
        Couldn’t load tasks: {state.error}
      </p>
    );
  }

  if (state.tasks.length === 0) {
    return <p className="tasks__status">No tasks yet — create your first one above.</p>;
  }

  return (
    <ul className="tasks__list" aria-label="Tasks">
      {state.tasks.map((task) => (
        <li key={task.id} className="tasks__item">
          <span className="tasks__item-title">{task.title}</span>
          <span className={`tasks__badge tasks__badge--${task.status}`}>
            {STATUS_LABELS[task.status]}
          </span>
        </li>
      ))}
    </ul>
  );
}
