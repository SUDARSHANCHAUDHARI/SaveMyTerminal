# SaveMyTerminal Phase 2: Persistence And Dashboard Design

**Status:** Approved through the existing V1 product design

## Summary

Phase 2 adds durable, privacy-safe session summaries and an embedded local dashboard to the Phase 1 service. It preserves the existing event protocol and process-wrapper privacy boundary: live events remain in memory, only approved summary fields reach SQLite, and normal operation makes no outbound network requests.

The dashboard opens automatically through `smt dashboard`, shows live sessions and 30-day history, and supports deleting one summary or purging all history. Browser clients authenticate without receiving the long-lived installation token.

## Scope

Phase 2 includes:

- SQLite schema creation and migrations.
- Persisted session headers and finalized summaries.
- Recovery of unfinished sessions as `interrupted` after service restart.
- Persistence enabled by default with 30-day retention.
- Cleanup at startup and periodically while the service is active.
- Authenticated APIs for active sessions, history, statistics, deletion, and purge.
- Authenticated Server-Sent Events for live snapshots.
- An embedded, responsive dashboard with active and history views.
- `smt dashboard`, which starts or reuses the service and opens the browser automatically.
- Graceful fallback to live-only operation if storage initialization or migration fails.

Phase 2 does not include:

- The Phase 3 configuration command or settings UI.
- Native agent hooks or context telemetry not yet produced by Phase 1.
- Native terminal rendering.
- Cloud synchronization, accounts, analytics, or update checks.
- GitHub Actions workflows.

## Chosen Approach

### Storage Library

Use `rusqlite` with bundled SQLite. Database work runs through a focused storage repository and `tokio::task::spawn_blocking` so synchronous SQLite calls do not block the async service runtime.

Alternatives considered:

- `sqlx`: strong async and compile-time query support, but adds a larger dependency and migration/tooling surface than this local single-process database needs.
- An append-only JSON store: simpler initially, but weak for retention, deletion, aggregation, migrations, and crash-safe updates.

### Dashboard Assets

Compile plain HTML, CSS, and JavaScript into the Rust binary with `include_str!`. The dashboard has no Node.js toolchain and no remote fonts, scripts, images, or stylesheets.

Alternatives considered:

- React or another frontend framework: unnecessary build and dependency complexity for two compact local views.
- Server-rendered HTML only: viable for history, but less suitable for live session updates and interactive deletion without full page reloads.

### Live Transport

Use Server-Sent Events. The service only needs to publish snapshots from server to browser; mutations continue through ordinary authenticated HTTP requests.

WebSockets are not required because Phase 2 has no bidirectional realtime protocol.

## Architecture

Phase 2 adds four bounded units:

1. `storage`: owns migrations, parameterized SQLite operations, retention, recovery, deletion, and aggregate queries.
2. `session coordinator`: applies validated events to the in-memory registry, persists permitted state, and publishes snapshots after successful registry mutation.
3. `dashboard auth and API`: creates one-time launch tokens, manages browser sessions, rejects cross-origin mutation requests, and exposes live/history endpoints.
4. `dashboard assets`: serves embedded same-origin HTML, CSS, and JavaScript.

The Phase 1 `SessionRegistry` remains the authority for live state transitions. Storage never accepts raw event JSON and cannot persist fields outside the summary model.

## Data Model

### Persisted Session Summary

The initial schema stores only fields currently available from Phase 1 plus nullable columns reserved for approved future telemetry:

- `session_id`: random UUID primary key.
- `agent_id`: validated fixed identifier.
- `adapter_id`: validated fixed identifier.
- `renderer_id`: nullable fixed identifier.
- `started_at_ms` and `updated_at_ms`.
- `ended_at_ms`: nullable while active.
- `duration_ms`: nullable while active.
- `final_state`: current or terminal normalized state.
- `failure_category`: nullable sanitized enum.
- `exit_code`: nullable integer.
- `transition_count` and `tool_event_count`.
- CPU sample count, average, and peak.
- Memory sample count, average, and peak.
- Context peak, final value, quality, and source as nullable fields until a later adapter supplies them.
- `adapter_kind`: `generic` or `native`.
- `finalized`: boolean represented as an integer.

The table contains no free-form metadata or JSON blob column. This makes prohibited data structurally difficult to store.

### Schema Versioning

Use SQLite `PRAGMA user_version` and ordered in-code migrations. Migrations execute in a transaction. An unsupported newer schema or failed migration disables persistence for that service process and records a concise metadata-only warning; live registry and dashboard operation continue.

### File Location And Permissions

Add a data directory to `AppPaths` and store the database as `sessions.sqlite3`. On Unix, newly created directories and database files use user-only permissions where supported. SQLite sidecar files remain in the same protected directory.

## Session Persistence Flow

1. A validated `Started` event creates the live snapshot.
2. The coordinator inserts a minimal persisted session header when persistence is available.
3. Lifecycle and metric events update in-memory aggregate counters and the permitted persisted columns.
4. A terminal event finalizes duration, final state, sanitized failure details, and aggregate metrics in one transaction.
5. Raw live event details are not stored.
6. The finalized snapshot is broadcast to dashboard clients.

Persistence failures are reported internally but never roll back a valid in-memory transition and never stop the wrapped child process.

## Restart Recovery

At storage startup, every row with `finalized = 0` is finalized as `interrupted`. Its end time is the last permitted update timestamp, duration is clamped to a non-negative value, and no synthetic prompt, output, command, or file data is created.

This recovery runs before history is served.

## Retention

Persistence is enabled by default with 30-day retention, matching the approved V1 design.

- Cleanup runs after migrations and restart recovery.
- While the service remains active, cleanup runs at most once per hour.
- Finalized summaries whose end timestamp is older than the retention cutoff are deleted.
- Active headers are never deleted by retention cleanup.
- Phase 3 will expose retention and persistence settings; Phase 2 uses the approved defaults internally.

## Live Publication

The coordinator publishes a complete active-session list after each accepted event. A `tokio::sync::watch` channel is sufficient because dashboard clients need the latest coherent state, not replay of every intermediate event.

The SSE endpoint:

- Sends an initial snapshot immediately.
- Sends later snapshots as `sessions` events.
- Sends periodic comment heartbeats to detect disconnected clients.
- Does not include installation tokens or prohibited data.
- Tracks connected dashboard clients so an open dashboard keeps the service alive.

When the last dashboard client disconnects and no event activity occurs for the configured idle timeout, the existing service shutdown policy resumes.

## Browser Authentication

The long-lived installation token remains available only to local CLI and adapter clients.

Dashboard launch flow:

1. `smt dashboard` authenticates to the service with the installation token.
2. It requests a cryptographically random, single-use launch token with a 60-second lifetime.
3. The CLI opens a loopback URL containing that launch token.
4. The service consumes the token, sets an `HttpOnly`, `SameSite=Strict`, path-scoped browser-session cookie, and redirects to `/dashboard` without a token in the URL.
5. Browser sessions are memory-only and disappear when the service exits.

API authentication accepts either the existing bearer token or a valid browser-session cookie. Mutation endpoints additionally require a same-origin `Origin` header when called with browser-cookie authentication. Responses include a restrictive Content Security Policy and `X-Content-Type-Options: nosniff`.

## HTTP Surface

Existing routes remain compatible:

- `GET /v1/health`
- `POST /v1/events`

Phase 2 adds:

- `POST /v1/dashboard-launch`: bearer-authenticated creation of a one-time launch URL.
- `GET /dashboard/launch`: consumes the launch token, sets the browser cookie, and redirects.
- `GET /dashboard`: serves the embedded shell.
- `GET /dashboard/app.css` and `GET /dashboard/app.js`: serve embedded assets.
- `GET /v1/sessions/active`: returns current nonterminal snapshots.
- `GET /v1/sessions/stream`: authenticated SSE live snapshots.
- `GET /v1/history`: returns newest finalized summaries with bounded pagination.
- `GET /v1/history/stats`: returns counts, completion-state totals, duration aggregates, context peaks when available, and resource aggregates.
- `DELETE /v1/history/{session_id}`: deletes one finalized summary.
- `DELETE /v1/history`: purges all finalized history.

Deletion never removes an active session header. Unknown IDs return `404`; attempts to delete active rows return `409`.

## Dashboard Experience

`smt dashboard` starts or reuses the service and opens the authenticated dashboard in the system browser. Failure to launch the browser prints the launch URL and returns an actionable error without stopping the service.

The landing page uses a compact two-view layout:

- **Live:** cards for active sessions showing agent, adapter, state, elapsed duration, CPU, memory, and metric quality/source.
- **History:** summary totals, completion-state distribution, duration/resource aggregates, and a newest-first session table.

Unavailable context or resource data renders as `Unavailable`; it is never shown as zero or inferred as exact. Destructive controls require an in-page confirmation. Purge requires a stronger second confirmation than deleting one row.

The dashboard is responsive, keyboard accessible, and functional without external assets. It displays a disconnected state when SSE drops and retries with bounded backoff.

## Error Handling

- Migration or database-open failure switches to live-only mode.
- Individual persistence writes are best effort after the in-memory transition succeeds.
- Storage errors return a stable `503 history_unavailable` API response while live endpoints remain healthy.
- Malformed pagination and identifiers return `400`.
- Unauthorized requests return `401`; rejected origins return `403`.
- Dashboard asset serving does not depend on SQLite availability.
- Browser-open failure does not invalidate the launch token immediately; the printed URL remains usable until expiry.

## Privacy Guarantees

Automated tests inspect the SQLite schema, stored rows, API JSON, SSE payloads, browser assets, and logs for prohibited field names and representative secret values.

The following remain prohibited everywhere:

- Prompt or response text.
- Terminal output.
- Raw commands or arguments.
- Environment values.
- Working-directory, repository, file-name, or file-content data.
- Hostnames, usernames, remotes, credentials, or API keys.

The dashboard performs no outbound requests. Its Content Security Policy permits only same-origin resources and connections.

## Testing Strategy

### Storage Tests

- Fresh database migration and idempotent reopen.
- Parameterized insert, update, finalization, and aggregate queries.
- Restart recovery to `interrupted`.
- Thirty-day retention boundary behavior.
- Single deletion, active-row conflict, and full purge.
- Migration failure producing live-only mode.
- Database schema and rows containing no prohibited columns or values.

### Service Tests

- Persistence follows successful registry mutation only.
- Persistence failure does not reject valid live events.
- Active/history/stat endpoints require authentication.
- Browser launch tokens are random, expiring, and single-use.
- Cookie-authenticated mutations enforce same-origin requests.
- SSE sends initial and changed snapshots and releases client liveness on disconnect.
- Open SSE clients prevent idle shutdown; disconnected clients do not.

### CLI And Dashboard Tests

- `smt dashboard` ensures the service and requests a short-lived launch URL.
- Browser-launch failure prints the usable URL.
- Embedded HTML references only local assets.
- Dashboard JavaScript renders exact, estimated, and unavailable metrics honestly.
- Delete and purge controls call only the intended authenticated endpoints.

### Regression Verification

All Phase 1 privacy, wrapper, service-startup, lifecycle, and exit-code tests continue to pass. Verification runs locally; Phase 2 does not add a GitHub Actions workflow.

## Completion Criteria

Phase 2 is complete when:

1. Finalized summaries survive service restarts in SQLite and contain only approved fields.
2. Unfinished persisted sessions recover as `interrupted`.
3. Thirty-day cleanup, single deletion, and purge are verified.
4. `smt dashboard` opens an authenticated loopback dashboard without exposing the installation token.
5. Live sessions update through authenticated SSE.
6. History and aggregate views work when storage is available and degrade clearly when it is not.
7. Dashboard clients participate correctly in service idle lifetime.
8. No outbound runtime request or GitHub Actions workflow is introduced.
9. All Phase 1 and Phase 2 local checks pass.
