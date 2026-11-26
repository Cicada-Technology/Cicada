<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="web/public/brand/logo-dark.svg" />
    <img src="web/public/brand/logo.svg" alt="cicada" width="420" />
  </picture>
</p>

# cicada

Production-grade Rust backend foundation.

Authentication · RBAC · data scopes · multi-tenancy · audit · code generation · observability

Batteries included. Write only your business modules.

[简体中文](README.md) · [English](README.en.md)

[![Rust](https://img.shields.io/badge/Rust-1.98-000000?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Axum](https://img.shields.io/badge/Axum-0.8-0f172a)](https://github.com/tokio-rs/axum)
[![SeaORM](https://img.shields.io/badge/SeaORM-2.0-0ea5e9)](https://www.sea-ql.org/SeaORM/)
[![PostgreSQL](https://img.shields.io/badge/PostgreSQL-17-336791?logo=postgresql&logoColor=white)](https://www.postgresql.org/)
[![Redis](https://img.shields.io/badge/Redis-7-DC382D?logo=redis&logoColor=white)](https://redis.io/)
[![Vue](https://img.shields.io/badge/Vue-3-42B883?logo=vue.js&logoColor=white)](https://vuejs.org/)
[![License](https://img.shields.io/badge/License-MIT-blue)](#license)

![cicada admin dashboard](docs/assets/dashboard.png)

## What is cicada

cicada is a production-grade backend foundation written in Rust. It ships the parts every back-office system needs and usually gets wrong once: authentication and sessions, RBAC, data scopes, multi-tenancy, audit, file storage, notifications, scheduled jobs, code generation, and observability. Your application only writes its own modules.

It is not a demo, and it is not a repository you fork. Framework crates are dependencies; business code lives in `business/`; upgrades change a version number.

**Who it is for:**

- teams building admin panels, operations platforms, or SaaS foundations in Rust;
- projects that need RBAC + data scopes + multi-tenancy + audit without rebuilding the plumbing;
- architectures that want a **modular monolith** now and a path to split services later through ports and adapters.

## Why cicada

- **Standard REST contract**: success returns plain JSON with real HTTP status codes; failures use RFC 9457 `problem+json` with machine-readable business codes, and the OpenAPI 3.1 snapshot is enforced in CI.
- **Production defaults**: audit outbox, partitioned and archived logs, rate limiting, graceful shutdown, metrics, OTLP tracing, and health probes are already wired in.
- **Compile-time correctness**: strongly typed IDs, typestate-based scoped queries, permission and audit macros. No reflection, no runtime string conventions. Missing permissions, tenant filters, or soft-delete filters are pushed to compile time or tests.
- **Modular monolith**: L0–L4 layers with an enforced dependency direction (`deploy/check_layering.py`). Cross-module collaboration goes through `ports` and domain events, not shared implementations.
- **Template-first, no fork**: `cicada new` generates a standalone project that depends on the framework. Business and framework code stay physically separate.
- **Four surfaces**: Rust REST API, Vue 3 admin UI, and one Tauri 2 native client for desktop / Android / iOS. All reuse the same pages and REST contract; tokens live in platform secure storage, and OAuth completes in the system browser via a `cicada://` deep link. Error messages are bilingual, and the shell supports zh-CN / en-US.

## Capabilities

| Area | What you get |
| --- | --- |
| **Authentication & sessions** | JWT + Redis sessions, refresh-token rotation, captcha, login lockout, TOTP, forced logout, online-session snapshots and history |
| **RBAC** | Permission strings, menu tree, role merging, versioned authorization cache, startup-time permission reconciliation |
| **Data scopes** | Five scopes (all / custom / own department / department and descendants / self only), typestate queries, optional PostgreSQL RLS fallback |
| **Multi-tenancy** | `tenant_id` across entities and cache keys, tenant packages, seat/storage/QPS quotas; tenant picker on login and transactional tenant provisioning with an initial admin |
| **System domain** | Users, roles, menus, departments, posts, dictionaries, config, notices, online users, operation and login logs |
| **Audit** | Operation/login logs and field-level change history; `outbox_event` + relay; monthly log partitions archived before deletion |
| **File storage** | One `Storage` trait for local / RustFS / Alibaba Cloud OSS / S3; magic-number checks; one-time download tokens or presigned URLs |
| **Notifications** | Email (SMTP) / webhook / SMS / IM gateways; templates, channels, delivery records; notice read state and header bell |
| **Scheduled jobs** | Cron triggers × inline/queued execution; cluster leader election prevents duplicate runs; online CRUD and execution logs |
| **Code generation** | Infer entity / DTO / handler / module / migration / frontend page from a table; single-table and tree templates; overridable templates; Rust artifacts compile in CI and frontend artifacts are typechecked |
| **Import/export** | Streaming import/export; large exports go through `sys_export_task`; 100k-row memory stability is covered by a test |
| **Open API** | API keys with issue/verify/revoke and `scopes` mapped into the permission set |
| **Request pipeline** | request_id, trace, security headers, CORS, gzip, body limits, per-IP token bucket + tenant QPS, idempotency keys |
| **Observability** | Structured stdout logs, `/metrics` (QPS/latency quantiles/pool/cache/task results/error rates by code), OTLP traces with DB child spans, slow-query logs |
| **Data monitoring** | Read-only PostgreSQL database/table stats: size, pool, sessions, row estimates, scans, vacuum/analyze times |
| **Quality gates** | fmt, clippy, `cargo doc -D warnings`, layering, `cargo deny`, `cargo machete`, OpenAPI snapshot, template compile, frontend lint/typecheck/build, image smoke test |

## Interface

Create and edit flows use the same dialog form; required-field and server-side errors are shown inline.

![Create user dialog](docs/assets/form-dialog.png)

## Architecture

```mermaid
flowchart TB
    UI["Vue 3 admin UI<br/>TypeScript · Element Plus · Pinia · Lucide"]
    API["External systems / Open API"]

    subgraph L4["L4 · Inbound"]
        WEB["cicada-web<br/>axum · middleware · OpenAPI"]
        WORKER["cicada-worker<br/>cron · apalis"]
    end

    subgraph L3["L3 · Capabilities"]
        AUTH["auth"]
        RBAC["rbac"]
        AUDIT["audit"]
        STORAGE["storage"]
        CODEGEN["codegen"]
    end

    subgraph L2["L2 · Domains"]
        SYSTEM["system"]
        MONITOR["monitor"]
        JOBS["jobs"]
        NOTIFY["notify"]
    end

    subgraph L1["L1 · Infrastructure"]
        INFRA["infra<br/>config · db · cache · telemetry · ratelimit · rls"]
        PORTS["ports<br/>directory · events · jobs · locks"]
    end

    subgraph L0["L0 · Kernel"]
        KERNEL["kernel<br/>errors · typed IDs · context · permissions · scopes"]
    end

    PG[("PostgreSQL 17")]
    REDIS[("Redis 7")]
    OBJ[("Local / RustFS / OSS / S3")]

    UI --> WEB
    API --> WEB
    WEB --> L3
    WEB --> L2
    WORKER --> JOBS
    L3 --> L1
    L2 --> L1
    L1 --> L0
    INFRA --> PG
    INFRA --> REDIS
    STORAGE --> OBJ
```

**Dependencies only point inward**: `L4 → L3/L2 → L1 → L0`. L2 domain modules must not depend on each other; this is enforced in CI. Cross-module collaboration uses `ports` and domain events.

## Quick start

### Prerequisites

- Rust 1.98+ (the repository ships `rust-toolchain.toml`)
- Docker + Docker Compose (PostgreSQL 17 / Redis 7)
- Node.js 22.12+ and pnpm 10 (frontend only)

### Local development

```bash
# 1. Start dependencies (not the compose "server" service; run it with cargo below)
docker compose up -d postgres redis

# 2. Schema → default tenant/admin → seed (demo rows attach to the existing admin/tenant)
cargo run -p cicada-cli -- migrate
cargo run -p cicada-cli -- init-admin
cargo run -p cicada-cli -- seed   # dict/config/menu + demo org/notice/packages

# 3. Start the backend
cargo run -p cicada-server
```

The backend listens on `http://localhost:8081` by default.

### Frontend

```bash
cd web
pnpm install
pnpm dev
```

Open `http://localhost:5173`. The dev server proxies `/api` and `/auth` to `http://127.0.0.1:8081`, so no CORS setup is required.

### Native client (Tauri: desktop + Android + iOS)

```bash
cd clients/app
corepack pnpm install
corepack pnpm dev        # desktop
corepack pnpm build      # desktop installers
corepack pnpm android:dev
corepack pnpm android:build -- --apk
corepack pnpm ios:dev    # macOS + Xcode only
corepack pnpm ios:build
```

Desktop, Android, iOS and Web reuse the same `web/` pages and REST contract. The shell only adds
platform secure storage (`dev.cicada.client`; Keychain, Android Keystore, etc.), file saving, and
OAuth in the system browser returning via `cicada://oauth/callback`. See
[`clients/app/README.md`](clients/app/README.md) for mobile prerequisites and signing.

### Everything in Docker

```bash
docker compose up -d --build
docker compose exec server cicada-cli migrate
docker compose exec server cicada-cli init-admin
docker compose exec server cicada-cli seed
```

The container listens on 8081 and is published on `http://localhost:8081`; the same-origin admin
UI is served by the `web` service at `http://localhost:8088` (Nginx proxies `/api`, `/auth`, `/docs`).

### First login

| Item | Value |
| --- | --- |
| Username | `admin` |
| Password | `Cicada@2026` |
| Login endpoint | `POST /auth/login`, body: `{"username":"admin","password":"Cicada@2026"}` |
| Admin UI | `http://localhost:5173` in development; in production serve `web/dist` from Nginx/CDN |

> **Change the default password before going live.** Use `cicada-cli admin reset --user admin --password '<new-password>'`, or reset it from the user management page.

### Common endpoints

| Endpoint | Purpose |
| --- | --- |
| `GET /healthz` | Process liveness |
| `GET /readyz` | Readiness probe (checks PostgreSQL and Redis) |
| `GET /metrics` | Prometheus text metrics |
| `GET /docs` | Swagger UI |
| `GET /api-docs/openapi.json` | OpenAPI 3.1 document |
| `POST /auth/login` | Login; returns access and refresh tokens |
| `/api/v1/...` | Business API (uniform response contract) |

## Project layout

```text
crates/
  kernel/     L0: errors, typed IDs, RequestContext, permissions, scoped-query state machine
  infra/      L1: config / db / cache / telemetry / ratelimit / rls
  ports/      L1: cross-module ports (directory, events, jobs, locks) and in-memory adapters
  system/     L2: users / roles / menus / departments / posts / dictionaries / config / notices / logs
  monitor/    L2: online users, cache monitoring, server metrics
  jobs/       L2: job definitions and execution logs
  notify/     L2: notification channels, templates, delivery records
  auth/       L3: identity resolution, passwords, sessions, JWT, OAuth/OIDC, TOTP, captcha
  rbac/       L3: permission strings, role merging, data-scope evaluation, authorization cache
  audit/      L3: audit events, field-level changes, transactional outbox
  storage/    L3: Storage trait and local / S3 / OSS / RustFS drivers
  codegen/    L3: table discovery, type inference, template rendering, zip packaging
  web/        L4: axum routes, middleware chain, module registry, OpenAPI
  worker/     L4: job runtime (cron + apalis)
  macros/     Permission/audit proc macros and DataScoped derive
  all/        Facade crate `cicada`: bootstrap + layer aliases + feature forwarding
  testkit/    Test fixtures (temporary PG, login helpers, assertion macros)
apps/
  server/     The framework's own assembly root
  cli/        migrate / seed / init-admin / tenant / admin reset / rls / gen / new
template/     Project template rendered by `cicada new`
migrations/   sea-orm-migration project (builds schema from empty; append-only)
web/          Vue 3 admin UI (built independently)
clients/
  app/        Tauri 2 + Rust + TS client (desktop + Android/iOS, reuses web/)
config/       Layered configuration (default / profile / local)
deploy/       Dockerfile, compose, init scripts, layering check
docs/         Design docs, ADRs, upgrade guide, OpenAPI snapshot
```

## Create your own project (without forking)

```bash
# Depend on a git tag (recommended)
cargo run -p cicada-cli -- new my-app \
  --cicada 'git = "https://github.com/Cicada-Technology/cicada", tag = "v0.1.0"'

# Or depend on a local workspace while developing the framework
cargo run -p cicada-cli -- new my-app --name my_app --cicada path=$PWD

cd my-app
cargo run --bin migrate -- up --seed --admin
cargo run
```

The generated project contains:

- `business/`: business modules (where the code generator writes);
- `migrations/`: business migrations;
- `templates/`: code-generation template overrides;
- `config/`: project configuration;
- `web/`: the same Vue admin UI (copied by `cicada new`);
- `clients/app/`: the Tauri 2 native client (desktop + Android/iOS) reusing `web/`.

Run `corepack pnpm android:init` (Android SDK/NDK) or, on macOS, `corepack pnpm ios:init`
inside `clients/app` before the first mobile build so Tauri generates `gen/` for this project.

Framework crates remain dependencies, so upgrades change only the version. `cargo-generate` only
renders the Rust template and does not copy `web/` or `clients/app/`; use `cicada new` for the
full-stack layout (or copy those two directories manually).

## Configuration

Configuration is layered; later layers override earlier ones:

```text
code defaults < config/default.yaml < config/{profile}.yaml < config/local.yaml < CICADA_* environment variables
```

- `CICADA_PROFILE=prod` selects `config/prod.yaml`;
- `config/local.yaml` is for local overrides (gitignored);
- nested keys use `__`, for example `CICADA_AUTH__JWT__PRIVATE_KEY`;
- `DATABASE_URL` and `REDIS_URL` are named variables with higher precedence.

At minimum, review these for production:

```bash
CICADA_PROFILE=prod
DATABASE_URL=postgres://user:pass@host:5432/cicada
REDIS_URL=redis://host:6379/0
CICADA_AUTH__JWT__PRIVATE_KEY="$(cat jwt-private.pem)"
CICADA_AUTH__JWT__PUBLIC_KEY="$(cat jwt-public.pem)"
CICADA_STORAGE_ACCESS_KEY_SECRET=...
```

`profile=prod` rejects the example JWT key, short HS256 secrets, and unsafe defaults. EdDSA (Ed25519) is recommended; configure CORS, storage, and the OTLP endpoint as needed.

## Testing and quality gates

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
# Requires real PostgreSQL / Redis: docker compose up -d postgres redis
cargo test --workspace
cargo test --workspace -- --ignored
cargo test -p cicada-migrations --test roundtrip -- --ignored

# Dependencies and supply chain
python3 deploy/check_layering.py
cargo deny check
cargo machete

# Frontend
cd web
pnpm lint
pnpm typecheck
pnpm build
```

CI has four jobs: **rust** (fmt / clippy / doc / unit / integration / migration roundtrip / layering / deny / machete / OpenAPI snapshot), **template** (`cicada new` → compile → migrate), **frontend** (lint / typecheck / build), and **image** (build → migrate/seed → smoke → health check).

API changes require regenerating the OpenAPI snapshot and keeping the diff in the PR:

```bash
CICADA_OPENAPI_BLESS=1 cargo test -p cicada-web --test openapi_snapshot
```

## Security checklist

- Change the default admin password; remove or disable unused bootstrap accounts.
- Run with `CICADA_PROFILE=prod`, configure EdDSA keys, and rotate `kid`.
- Inject database, Redis, and object-storage credentials from the environment or a secret manager; never commit them.
- Enable CORS only when the frontend is on a different origin, and list explicit origins.
- Enable RLS fallback for high-risk tenant tables when appropriate (`cicada-cli rls enable --table ...`).
- Tune audit retention and archival to your compliance requirements (`audit.retention_days`).

## Status and roadmap

Current version: **0.1.0**. The core foundation is delivered:

- ✅ Authentication, RBAC, data scopes, multi-tenancy (login picker + provisioning), system domain, audit, storage, notifications, jobs, code generation, import/export, open API, observability;
- ✅ Vue admin UI + Tauri 2 native client (desktop / Android / iOS); profile, self-service password, TOTP, file manager, export tasks, change history and the notice bell are closed loops;
- Out of scope: Kubernetes, master-detail code generation and the form builder; their menus/claims have been removed.

Migrations are append-only. Breaking changes to APIs and configuration are recorded in [`CHANGELOG.md`](CHANGELOG.md); upgrade steps live in [`docs/UPGRADE.md`](docs/UPGRADE.md).

## Documentation

- [简体中文 README](README.md)
- [Design document](docs/superpowers/specs/2026-09-30-cicada-rust-base-design.md) (Chinese)
- [Architecture Decision Records](docs/adr/)
- [Upgrade guide](docs/UPGRADE.md)
- [Changelog](CHANGELOG.md)
- Per-crate responsibilities, public API, and boundaries: `crates/*/README.md`
- Runtime OpenAPI: `/docs` or `/api-docs/openapi.json`

> Source comments and design documents are primarily in Chinese; this English README mirrors the essentials.

## License

Released under the **MIT** license. See [LICENSE](LICENSE).
