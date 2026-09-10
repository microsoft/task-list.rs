import { useState, type FormEvent } from 'react';

import { validateTitle } from './title';

/**
 * Create-task form: a title input + submit, with client-side validation mirroring the
 * domain rule (trim; non-blank; 1..=200). Submit is disabled while the input is invalid or
 * a request is in flight. `onCreate` receives the trimmed title and resolves on success;
 * its rejection (e.g. a 400 problem+json) is surfaced inline.
 */
export function CreateTaskForm({ onCreate }: { onCreate: (title: string) => Promise<void> }) {
  const [title, setTitle] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);

  const validation = validateTitle(title);
  // Show the validation hint only once the user has typed (don't nag a pristine field).
  const validationHint = title.length > 0 && !validation.ok ? validation.error : null;
  const disabled = !validation.ok || submitting;

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!validation.ok) {
      return;
    }
    setSubmitting(true);
    setSubmitError(null);
    try {
      await onCreate(validation.value);
      setTitle('');
    } catch (error: unknown) {
      setSubmitError(error instanceof Error ? error.message : 'Failed to create task.');
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <form className="task-form" onSubmit={handleSubmit} noValidate>
      <label className="task-form__label" htmlFor="task-title">
        New task
      </label>
      <div className="task-form__row">
        <input
          id="task-title"
          className="task-form__input"
          type="text"
          value={title}
          placeholder="What needs doing?"
          onChange={(event) => setTitle(event.target.value)}
          aria-invalid={validationHint != null}
          aria-describedby={validationHint ? 'task-title-error' : undefined}
          disabled={submitting}
        />
        <button className="task-form__submit" type="submit" disabled={disabled}>
          {submitting ? 'Adding…' : 'Add task'}
        </button>
      </div>
      {validationHint && (
        <p id="task-title-error" className="task-form__error" role="alert">
          {validationHint}
        </p>
      )}
      {submitError && (
        <p className="task-form__error" role="alert">
          {submitError}
        </p>
      )}
    </form>
  );
}
