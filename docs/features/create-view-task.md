# Work Item: Create a task and show it
**Branch:** vibe/create-view-task
**Status:** Complete

> **Intent (Mr. Das's words):** *"Create a task and show it."* The first real product feature on top of the
> walking skeleton: a user creates a task and sees it displayed. **Out of scope — deferred to later
> features:** updating tasks, deleting tasks, associating images. Keep the slice tight to **create + view**.
>
> **Source of truth:** `docs/design.md` (the merged authoritative design). This spec sequences the feature;
> where they differ, design.md wins.

---

## Baseline (what already exists — so we build only the gap)

The carried-forward Cosmos spike (from bootstrap, uncommitted into this branch) already provides the
persistence foundation:

- **domain:** `Task{id, owner, title, status, created_at, updated_at, version: Option<ETag>}`; newtypes
  `TaskId`(uuidv7) / `UserId` / `Title`(trimmed, 1..=200, non-blank) / `ETag`; `TaskStatus{Todo(default),
  InProgress, Done}`. **Sufficient for create+view as-is.**
- **application:** `TaskRepository` port already has **`create` / `get` / `list_for_owner` / `update`**;
  `Clock`; `ApplicationError{Domain→400, NotFound→404, Conflict→409, Repository→500}`; `TaskDto` /
  `TaskStatusDto`. Only `tasks::list` use-case exists so far.
- **infrastructure:** `InMemoryTaskRepository` (full create/get/update, default/test/CI) **and**
  `CosmosTaskRepository` (full create/get/update, **proven etag round-trip on the emulator**);
  `Persistence{Memory(default), Cosmos}` via `TASKLIST_PERSISTENCE`.
- **api:** `GET /api/tasks` (owner-scoped list through the port) is live; `POST` is not. RFC7807 maps
  400/404/409/500. `AppState.demo_owner`.
- **web:** `apiFetch<T>` wrapper (supports POST), `api` façade, generated client + drift check. App shows
  health/auth cards only.

**Net gap:** a `create` use-case + DTO, `POST /api/tasks`, regen typed client, a create-form + list UI, and
the local-dev Cosmos gate/docs. The backend `create` port + both adapters are already done — a genuinely
thin slice.

---

## Options Considered

### Option 1 — Minimal list slice  ✅ RECOMMENDED (SELECTED)
- **Description:** Add `POST /api/tasks` (201 + `TaskDto`); reuse the existing `GET /api/tasks` for "show
  it." Web = one Tasks page: create-form above a task list; after create, optimistically append. No detail
  route, no `GET /{id}`.
- **Pros:** Smallest true end-to-end slice — the list endpoint is already free, so backend = one handler +
  one use-case. Fully satisfies "create + show." Exercises the proven Cosmos path. Nothing speculative.
- **Cons:** No single-item view yet; update/delete (later features) will want a detail route then.

### Option 2 — List + detail slice
- **Description:** Option 1 plus `GET /api/tasks/{id}` + a detail route; redirect to the new task after create.
- **Pros:** Pre-builds routing that update/delete will need; a focused single-item view; `Location` becomes meaningful.
- **Cons:** A second endpoint, client routing, and a second data hook now — scaffolding for *later* features (YAGNI for create+view). **Rejected** for this slice.

### Option 3 — Create-only echo (thin)
- **Description:** `POST /api/tasks` echoes the created task; web shows just the just-created item inline; keep in-memory, don't emphasize persistence.
- **Pros:** Fastest to build.
- **Cons:** "Show it" is weak — a refresh loses the view; ignores the free list endpoint and the proven Cosmos path. **Rejected.**

## Selected Design

**Option 1 — minimal list slice.**

- **domain:** no change (create+view fully covered by existing `Task::new` / `Title::parse` /
  `TaskStatus::default()==Todo`).
- **application:** new DTO `CreateTaskRequest{ title: String }` (`Deserialize` + `ToSchema`); new use-case
  `tasks::create(repo, clock, owner, req) -> Result<TaskDto>` — `Title::parse` (invalid → `Domain`→400),
  server-mint `TaskId::new()`, status = `Todo`, `Task::new(..clock.now())`, `repo.create` → `TaskDto`.
  Owner-scoped to the caller (`demo_owner` until Auth). Unit tests vs the in-memory fake (happy, blank/oversize
  title, default status, owner isolation).
- **api:** `POST /api/tasks` → **201 Created**, body `TaskDto` (no `Location` header this slice); utoipa
  `(201, 400)`; register schema; **regenerate `web/openapi.json` + `web/src/ApiClient.generated.ts`** (drift
  check green). axum HTTP tests: 201 happy, 400 invalid title (RFC7807 shape), create-then-listed.
- **web:** add `createTask(req)` to the façade (POST JSON via `apiFetch`); a Tasks page with a create-form
  (title input; client-side required/length validation **mirroring the domain rule** 1..=200/non-blank) above
  a task list with **loading / empty / error** states; **on success, optimistically append** the returned
  task. Replaces the skeleton cards. Tests: form validation + submit (client mocked), list render states;
  timing-safe.
- **persistence wiring:** **code/test/CI default stays `Persistence::Memory`** (hermetic — no emulator needed
  for `cargo test`/CI). The running local dev app **also defaults to in-memory**; **Cosmos is opt-in** via
  `TASKLIST_PERSISTENCE=cosmos`.
- **offline / PWA:** explicitly **out of scope** — no service worker, no IndexedDB, no outbox, no
  client-minted ids, no conflict handling. Online create + view only. (The `update`/`Conflict`/etag machinery
  stays dormant.)

---

## Task Breakdown

Each row is a full-stack, independently verifiable slice. Dave signals `building`→`ready` per task;
Bhaskar validates; Anders design-reviews; JARVIS handles all git.

| # | Task Description | Status | Commit |
|---|------------------|--------|--------|
| 0 | **Persistence foundation (carried from the Cosmos spike).** Commit the already-built `TaskRepository` create/get/update + `CosmosTaskRepository` + `Task.version` ETag + `ApplicationError::Conflict`(409) + CI integration job (Linux emulator container) that came over from bootstrap. Verify green (fmt/clippy/tests, drift). | ✅ Done | f1fbcd4 |
| 1 | **Backend create endpoint.** application: `CreateTaskRequest` DTO + `tasks::create` use-case (validate title, mint id, default `Todo`, clock, `repo.create`) + unit tests vs in-memory fake. api: `POST /api/tasks` → 201 `TaskDto` (400 on invalid), utoipa path + schema, regen `openapi.json` + typed client (drift green), axum HTTP tests (201 / 400 / then-listed). | ✅ Done | (feature PR) |
| 2 | **Create form + task list UI.** web: `createTask` in the façade (+POST JSON in `apiFetch`); Tasks page — create-form (client-side title validation mirroring the domain) + list with loading/empty/error, **optimistic append** on success; replace skeleton cards. Unit tests for form + list states. | ✅ Done | (feature PR) |
| 3 | **Local-dev Cosmos gate + native-Windows docs.** `session-startup.ps1`: a **conditional** preflight that waits for the Cosmos emulator only when the local app runs in Cosmos mode (`TASKLIST_PERSISTENCE=cosmos`) — never blocks the default in-memory loop; probe the **native Windows Cosmos DB Emulator** (`https://localhost:8081`). README: **native Windows** emulator install/start steps. No domain/app/api code; hermetic tests/CI unchanged. | ✅ Done | (feature PR) |

*(Task 0 lands the carried-forward foundation; Tasks 1–2 are verifiable on the in-memory default immediately;
Task 3 adds the opt-in Cosmos gate + docs.)*

## Global Refactoring Log

_(from the carried-forward Cosmos spike — to be committed in Task 0)_
- **domain:** `Task` gained `version: Option<ETag>` (additive; `Task::new` stamps `None`); new `ETag` newtype.
- **application (port):** `TaskRepository` grew `get(owner,id)` + `create(&task)` + `update(&task)` alongside `list_for_owner`; `update` carries the etag for optimistic concurrency.
- **application (error):** added `ApplicationError::Conflict` (→409).
- **api:** RFC7807 mapping gained the 409 arm.
- **config/composition:** `Persistence` enum (`TASKLIST_PERSISTENCE`, default `memory`) + `CosmosSettings`; `TASKLIST_ENV=local`→emulator, `cloud`→token credential; `build_state` selects Cosmos vs in-memory.
- **CI:** a separate `integration` job runs the Cosmos **Linux** emulator as a service container (CI stays on Linux runners); local dev uses the **native Windows** emulator.

## Review Log

### Task 1 — backend create endpoint (2026-07-09)
- **Bhaskar (verify): PASS** — fmt/clippy(-D warnings)/tests green; **+8 tests** (application 7, api http 7);
  release build ✓; OpenAPI drift **byte-identical** (client regenerated, not hand-edited); full web gate
  (lint/typecheck/vitest/build) ✓; live smoke confirms contract (201 trimmed + Todo + no Location; 400
  problem+json); **no layering violation** (`application` has no `infrastructure` dep — in-crate FakeRepo).
- **Anders (design review): APPROVE-WITH-SUGGESTIONS** — faithfully realizes Option 1; rulings honored.
  - **Finding 1 (⏳ Mr. Das's ruling):** structural body rejections bypass RFC7807 — axum's `Json` extractor
    returns non-problem+json on missing `title` (422 text/plain), malformed JSON (400 text/plain), missing
    `Content-Type` (415 text/plain). Only the *domain-invalid* title path yields 400 problem+json. This is the
    **first mutation endpoint**, so it's the moment to set the shared `JsonRejection → RFC7807` seam once.
    **Not a blocker** for Task 1's stated contract. Anders rec: **fix now**.
  - **Finding 2 (Anders ruled):** keep default/lenient serde on `CreateTaskRequest` (no `deny_unknown_fields`) —
    liberal-in-what-you-accept, forward-compatible. No change.
  - **Finding 3 (carry-forward to Task 2):** `createTask` MUST send `Content-Type: application/json` +
    `JSON.stringify(body)` or axum returns 415. (Today's `apiFetch` sets only `Accept`.)
  - Bonus: the 201 returns a **fully server-stamped `TaskDto`**, so Task 2's optimistic append can use it
    directly — no refetch needed.
- **Finding 1 — RULED fix-now (Mr. Das) & DONE.** Dave added a reusable RFC7807-on-body-rejection seam:
  `From<JsonRejection> for ApiError` + a thin `crate::extract::Json<T>` wrapper (both `create_task` and
  `list_tasks` route through it, so every future mutation endpoint inherits it). Mapping (all problem+json):
  missing `Content-Type`→415; malformed JSON / missing-blank-wrong-type field / bytes→400 (no bare 422);
  201 + domain-invalid-400 unchanged. OpenAPI documents 201/400/415; client regenerated. **Bhaskar re-verify:
  PASS** (api http 7→10, drift byte-identical, web gate green, seam genuinely shared, no layering regression).

### Task 2 — create-form + task list UI (2026-07-09)
- **Bhaskar (verify): PASS** — web gate green (lint/typecheck/**test:ci 27/27**/build); backend + generated client +
  openapi.json untouched (empty diff); behavior confirmed: Content-Type only on bodied requests + RFC7807 detail
  surfaced; `validateTitle` mirrors the domain rule (trim/non-blank/1..=200 **code points**, incl. emoji ×200/×201
  boundary); **append-on-success** (no refetch, list untouched + input preserved on failure); four list states;
  mobile-first CSS; no flaky tests (guardrail #8).
- **Anders (design review): APPROVE-WITH-SUGGESTIONS** — faithful to Option 1; clean frontend layering (pure
  validation / data hook / presentational / apiFetch seam); Finding 3 (Content-Type) correctly at the seam.
  - **Finding 1 (fixed + re-verified):** create-during-initial-load race in `useTasks` — a stale initial load
  could clobber a just-created task (widened by scale-to-zero cold starts). Dave guarded it (functional
  setState keyed on `prev.status`: stale load **merges deduped-underneath**; stale load **error can't mask** a
  create) — form stays usable during cold start. +2 deterministic tests (**29/29**). **Bhaskar re-verify: PASS**.
  - **Finding 2 (Anders ruled, accept):** client/server title-validation duplication is intentional UX mirroring;
  server stays the boundary; degrades gracefully. *(Optional deferred: utoipa `#[schema(max_length=200)]` if drift ever bites.)*
  - **Findings 3–4 (forward notes):** the `apiFetch`/`useTasks` seam is the correct pivot for the future offline
  model (client-minted uuidv7 + outbox will slot behind it with no presentational change); `ApiError` keeps
  `status` so a future 409 conflict flow can branch. No action now.

## Notes & Decisions

### Task 3 — conditional Cosmos wait-gate + native-Windows docs (2026-07-09)
- **Bhaskar (verify): PASS** — scope confined to 3 files (`scripts/cosmos-preflight.ps1` new, `session-startup.ps1`,
  `README.md`); backend **45** / web **29** green; OpenAPI drift clean; in-memory default skips in 126ms (no probe),
  cosmos-down emits the actionable message and times out **advisory-only (never blocks the watch)**;
  case-insensitive mode resolution + full caller-env restore; wiring non-invasive (health/dev-state/lifecycle
  contracts untouched); cert claim verified against `cosmos.rs`. No watch left running.
- **Anders (design review): APPROVE** — closes the feature design-wise. Conditional gate faithfully realizes the
  in-memory-default / Cosmos-opt-in reconciliation; env hygiene + fresh-boot wiring correct; native-Windows-local
  vs Docker-Linux-CI split coherent.
  - **Finding 1 (confirmed w/ Mr. Das):** default local "show it" is **ephemeral** (in-memory) — durable across
    restarts is the one-line opt-in `TASKLIST_PERSISTENCE=cosmos`. This is the trade Mr. Das chose knowingly.
  - **Finding 2 (forward note):** local (native Windows) vs CI (Docker Linux) are distinct emulator builds — keep
    the *proving* etag/conflict integration tests pinned to the CI emulator; treat local as demo convenience.
  - **Finding 3 (Anders ruled, leave as-is):** the cosmos-down wall-clock overshoot (≤ one 5s probe past the
    deadline) is cosmetic/bounded — YAGNI to cap.

### Feature close-out
"Create a task and show it" is **complete** — Task 0 (Cosmos persistence foundation) → Task 1 (POST create endpoint
+ RFC7807 body-rejection seam) → Task 2 (create-form + list UI, append-on-success) → Task 3 (conditional local-dev
Cosmos gate + docs). Option 1 realized end-to-end. Cleanly deferred behind established seams: detail route,
update/delete, image association, offline/PWA.

## Notes & Decisions — original

**Rulings by Mr. Das (2026-07-08):**
- **View shape:** **list-only** (reuse `GET /api/tasks`); no detail route this feature.
- **Create inputs:** **title-only**; status defaults to **Todo**.
- **Post-create UX:** **optimistically append** the returned task to the list.
- **Local-dev persistence:** running local app **defaults to in-memory**; **Cosmos is opt-in**
  (`TASKLIST_PERSISTENCE=cosmos`). Code/test/CI stay in-memory + hermetic regardless. *(Tradeoff accepted:
  in-memory means created tasks don't survive an app restart unless run in Cosmos mode.)*
- **Emulator flavor:** **native Windows Cosmos DB Emulator** (Mr. Das installed it). The repo's Docker
  **Linux** emulator (`scripts/docker-compose.yml`) remains the **CI** path (Linux runners); local dev targets
  the native Windows emulator.
- **Emulator wait-gate reconciliation (JARVIS, from the two rulings above):** because local dev defaults to
  in-memory, the startup wait-for-emulator is **conditional** — it fires only in Cosmos opt-in mode, so it
  never blocks the default loop. README still documents the Windows install steps. *(Flagged to Mr. Das; veto
  if unconditional was intended.)*

**Adopted defaults (Anders' recommendations, unless vetoed):**
- Server mints `TaskId` in the create use-case (client-minted id / idempotent create deferred with the offline slice).
- Client-side title validation mirrors the domain rule (trimmed, 1..=200, non-blank).
- Single `demo` owner until Auth lands.
- 201 returns the created `TaskDto`; `Location` header deferred (only meaningful with a detail route).
- No CSRF/auth this feature (arrives with the Auth slice).

**Deferred to later features:** update, delete, image association; single-item detail view/route; offline/PWA
(client-minted ids, outbox, etag conflict UX); `description`/`due_date` fields.
