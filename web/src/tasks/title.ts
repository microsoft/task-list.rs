/**
 * Client-side title validation, mirroring the domain rule enforced by `Title::parse`
 * (trim; non-blank; 1..=200 characters). Length is measured in Unicode code points
 * (`[...s].length`) to match the backend's `chars().count()`, not UTF-16 code units.
 *
 * This is the single source of truth for the form's inline feedback; the server re-checks
 * the same invariant, so this only improves UX — it is not the security boundary.
 */
export const MAX_TITLE_LENGTH = 200;

export type TitleValidation = { ok: true; value: string } | { ok: false; error: string };

export function validateTitle(raw: string): TitleValidation {
  const trimmed = raw.trim();
  const length = [...trimmed].length;
  if (length === 0) {
    return { ok: false, error: 'Title is required.' };
  }
  if (length > MAX_TITLE_LENGTH) {
    return { ok: false, error: `Title must be ${MAX_TITLE_LENGTH} characters or fewer.` };
  }
  return { ok: true, value: trimmed };
}
