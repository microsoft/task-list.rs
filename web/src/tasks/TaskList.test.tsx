import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import type { TaskDto } from '../api/client';
import { TaskList } from './TaskList';

function task(overrides: Partial<TaskDto> = {}): TaskDto {
  return {
    id: '1',
    owner: 'demo|user',
    title: 'Buy milk',
    status: 'todo',
    createdAt: '2026-07-09T00:00:00Z',
    updatedAt: '2026-07-09T00:00:00Z',
    ...overrides,
  };
}

describe('TaskList', () => {
  it('shows a loading state', () => {
    render(<TaskList state={{ status: 'loading' }} />);
    expect(screen.getByText('Loading tasks…')).toBeInTheDocument();
  });

  it('shows an error state', () => {
    render(<TaskList state={{ status: 'error', error: 'offline' }} />);
    expect(screen.getByRole('alert')).toHaveTextContent('offline');
  });

  it('shows an empty state when there are no tasks', () => {
    render(<TaskList state={{ status: 'ready', tasks: [] }} />);
    expect(screen.getByText(/No tasks yet/i)).toBeInTheDocument();
  });

  it('renders each task with its title and status label', () => {
    render(
      <TaskList
        state={{
          status: 'ready',
          tasks: [task({ id: '1', title: 'Buy milk', status: 'todo' }), task({ id: '2', title: 'Ship it', status: 'inProgress' })],
        }}
      />,
    );

    expect(screen.getByText('Buy milk')).toBeInTheDocument();
    expect(screen.getByText('To do')).toBeInTheDocument();
    expect(screen.getByText('Ship it')).toBeInTheDocument();
    expect(screen.getByText('In progress')).toBeInTheDocument();
    expect(screen.getAllByRole('listitem')).toHaveLength(2);
  });
});
