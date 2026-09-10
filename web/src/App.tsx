import { HealthIndicator } from './components/HealthIndicator';
import { AuthState } from './components/AuthState';
import { useHealth } from './hooks/useHealth';
import { CreateTaskForm } from './tasks/CreateTaskForm';
import { TaskList } from './tasks/TaskList';
import { useTasks } from './tasks/useTasks';

export default function App() {
  const health = useHealth();
  const tasks = useTasks();

  return (
    <main className="app">
      <header className="app__header">
        <h1 className="app__title">task-list.rs</h1>
        <p className="app__subtitle">Clean-architecture starter · create and view tasks</p>
      </header>

      <section className="card" aria-labelledby="tasks-heading">
        <h2 id="tasks-heading" className="card__title">
          Tasks
        </h2>
        <CreateTaskForm onCreate={tasks.createTask} />
        <TaskList state={tasks.state} />
      </section>

      <section className="card" aria-label="Service status">
        <HealthIndicator state={health} />
      </section>

      <section className="card" aria-label="Authentication">
        <AuthState />
      </section>
    </main>
  );
}
