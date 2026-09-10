# task-list.rs

Clean-architecture full-stack starter in **Rust + React**, built to light up the whole Azure stack
end-to-end. Developed by the 4-pack agentic loop.

## Features

- **Mobile-first, offline-first** installable PWA (offline/PWA layer lands in a later slice).
- **Clean architecture**: a Cargo workspace with a strict `domain ← application ← infrastructure/api`
  dependency flow and ports/adapters (dependency inversion) at every external seam.
- **Agentic development in action.**
- **Stack:** Rust (axum) · React 19 + TypeScript + Vite · GitHub Actions CI/CD · Containers ·
  Azure Container Apps · Cosmos DB · Azure Blob Storage · Auth0 (Google) · App Insights.

## How this was built — the agentic loop

Built by a small **self-improving, hub-and-spoke, human-in-the-loop** agentic loop — a crew of AI
agents that doesn't just build the product, it sharpens itself each cycle. **JARVIS** orchestrates and
every agent hands back to it; per task the flow is **Dave implements → Bhaskar verifies → Anders
design-reviews → Mr. Das decides**. Work happens on `vibe/<feature>` branches — `main` is never touched
directly — and JARVIS owns git and the PR.

- **Mr. Das** (human) — the visionary owner in the Iron Man mold: sets the direction, makes the final call, runs end-to-end testing, merges to `main`, and ships.
- **JARVIS** — the unflappable, dryly polite orchestrator (an AI butler of that same lineage); sequences hand-offs and owns git, the task file, and the PR.
- **Anders** — the exacting architect who reasons from first principles and won't let a layering violation slide; design and review only (no code, builds, or commits).
- **Dave** — the steady, pragmatic coder who just gets the task done (no commits/pushes).
- **Bhaskar** — the rigorous verifier who trusts nothing until the build, tests, and contracts prove it (no code, no commits).

The engine of that self-improvement is the periodic
**[retrospective](.github/skills/retrospective.md)**: every few features it distills what worked (and
what didn't) back into the guardrails and agent playbooks, so each cycle sharpens the next — JARVIS
reminds the human when one is due.

## Repository layout

```
domain/          Entities, value-object newtypes, enums, domain errors (zero inbound deps)
application/     Use-cases, port traits, DTOs, application errors (depends only on domain)
infrastructure/  Port adapters (in-memory now; Azure later) + config loader
api/             axum host: routers, OpenAPI, RFC7807, SPA serving, composition root
web/             React 19 + TS + Vite SPA
scripts/         Local-dev PowerShell helpers
docs/            design.md (project map + rules + full design), features/
Dockerfile       Multi-stage build → single image serving the API + SPA on one port
```

See **[`docs/design.md`](docs/design.md)** for the full design + hard rules.

## Prerequisites

- **Rust** ≥ 1.88 (stable), **Node.js** 22+, **Docker** (for the container build).
- Optional local tooling: `cargo install cargo-watch` (used by `scripts/dev.ps1`).

## Quick start

Clone, then run the whole local loop (API watcher + Vite) with `./scripts/dev.ps1` and open the
Vite dev URL it prints — the health indicator turns green off a live `GET /api/health`. API docs at
`/swagger`, spec at `/api/openapi.json`.

Full launch/restart/liveness details: **[run-app skill](.github/skills/run-app.md)** and
[`scripts/dev.ps1`](scripts/dev.ps1).

## Cosmos DB Emulator (native Windows) — optional local persistence

The default local loop uses the **in-memory** repository — nothing to install, and tests stay
hermetic. Persistence is opt-in: set `TASKLIST_PERSISTENCE=cosmos` and the API wires up the real
Cosmos adapter against the local **[Azure Cosmos DB Emulator](https://learn.microsoft.com/azure/cosmos-db/how-to-develop-emulator)**.
Locally we use the **native Windows** emulator; **CI** uses the Docker Linux emulator
([`scripts/docker-compose.yml`](scripts/docker-compose.yml)) — that stays the CI path.

**1. Install** (one-off):

```powershell
winget install Microsoft.Azure.CosmosEmulator
# or download the MSI: https://aka.ms/cosmosdb-emulator
```

**2. Start it** (from the Start menu, or):

```powershell
& "$env:ProgramFiles\Azure Cosmos DB Emulator\Microsoft.Azure.Cosmos.Emulator.exe"
```

The emulator serves its gateway at the well-known endpoint **`https://localhost:8081`** and hosts a
Data Explorer at `https://localhost:8081/_explorer/index.html`. It uses a fixed, publicly documented
**well-known key** (not a secret), the same one the adapter falls back to locally:

```
C2y6yDjf5/R+ob0N8A7Cgv30VRDJIWEHLM+4QDU5DE2nQ9nDuVTqobD4b8mGGyPMbIZnqyMsEcaGQy67XIw/Jw==
```

**3. Opt local dev into Cosmos** — in `scripts/dev-secrets.local.ps1` add:

```powershell
$env:TASKLIST_PERSISTENCE = 'cosmos'   # opt in; unset/anything-else ⇒ in-memory (default)
# TASKLIST_ENV stays 'local' — that's what trusts the emulator cert + auto-provisions the DB/container.
```

With `TASKLIST_ENV=local` (the default) the API trusts the emulator's self-signed certificate
automatically and provisions the `tasklist` database + `tasks` container on the fly, so **no manual
certificate import is needed for the app itself**. (A browser hitting the Data Explorer may still show
a cert warning — cosmetic, and irrelevant to the Rust client.) The endpoint, database, and container
are overridable via `COSMOS__ENDPOINT` / `COSMOS__DATABASE` / `COSMOS__CONTAINER`.

**Startup wait-gate.** When (and only when) `TASKLIST_PERSISTENCE=cosmos`, the session bootstrap
([`scripts/session-startup.ps1`](scripts/session-startup.ps1) → [`scripts/cosmos-preflight.ps1`](scripts/cosmos-preflight.ps1))
waits for the emulator to be reachable before the app starts, and prints an actionable message if it
isn't up (bounded wait — it never hangs). On the default in-memory loop the preflight is a no-op, so
startup stays fast.

## Build & test

Commands live in the skills — single source of truth, not restated here:

- **[build-test skill](.github/skills/build-test.md)** — fast gate: fmt · clippy (`-D warnings`) ·
  unit tests · web lint/typecheck/test/build.
- **[build-test-full skill](.github/skills/build-test-full.md)** — full gate: adds the release build
  and the OpenAPI client **drift check**.

The committed OpenAPI spec + typed client (`web/src/ApiClient.generated.ts`) are regenerated and
drift-checked per the [build-test-full skill](.github/skills/build-test-full.md); see also
[`docs/design.md`](docs/design.md).

## Container

The multi-stage [`Dockerfile`](Dockerfile) builds a single image serving the API + SPA on port 8080
(`docker build -t tasklist . && docker run --rm -p 8080:8080 tasklist`).

## Conventions

Hard rules — dependency flow, zero-warnings, never editing generated files, and secret handling —
live in [`docs/design.md`](docs/design.md).
