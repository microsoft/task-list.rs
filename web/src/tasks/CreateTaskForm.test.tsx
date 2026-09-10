import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';

import { MAX_TITLE_LENGTH } from './title';
import { CreateTaskForm } from './CreateTaskForm';

function setup(onCreate = vi.fn().mockResolvedValue(undefined)) {
  render(<CreateTaskForm onCreate={onCreate} />);
  const input = screen.getByLabelText('New task') as HTMLInputElement;
  const submit = screen.getByRole('button', { name: /add task/i });
  return { onCreate, input, submit };
}

describe('CreateTaskForm', () => {
  it('disables submit for a pristine (empty) field', () => {
    const { submit } = setup();
    expect(submit).toBeDisabled();
  });

  it('keeps submit disabled and shows an error for a whitespace-only title', () => {
    const { input, submit } = setup();
    fireEvent.change(input, { target: { value: '   ' } });
    expect(submit).toBeDisabled();
    expect(screen.getByRole('alert')).toHaveTextContent(/required/i);
  });

  it('shows an error and disables submit for an oversize title', () => {
    const { input, submit } = setup();
    fireEvent.change(input, { target: { value: 'a'.repeat(MAX_TITLE_LENGTH + 1) } });
    expect(submit).toBeDisabled();
    expect(screen.getByRole('alert')).toHaveTextContent(/characters or fewer/i);
  });

  it('enables submit for a valid title', () => {
    const { input, submit } = setup();
    fireEvent.change(input, { target: { value: 'Buy milk' } });
    expect(submit).toBeEnabled();
    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });

  it('submits the trimmed title and clears the input on success', async () => {
    const { onCreate, input, submit } = setup();
    fireEvent.change(input, { target: { value: '  Buy milk  ' } });
    fireEvent.click(submit);

    await waitFor(() => expect(onCreate).toHaveBeenCalledWith('Buy milk'));
    await waitFor(() => expect(input.value).toBe(''));
  });

  it('surfaces the error and keeps the input when create fails', async () => {
    const onCreate = vi.fn().mockRejectedValue(new Error('title must be between 1 and 200 characters'));
    const { input, submit } = setup(onCreate);
    fireEvent.change(input, { target: { value: 'Buy milk' } });
    fireEvent.click(submit);

    await waitFor(() =>
      expect(screen.getByRole('alert')).toHaveTextContent(/between 1 and 200 characters/i),
    );
    expect(input.value).toBe('Buy milk');
  });
});
