# Phase 2 Persistence And Dashboard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add privacy-safe SQLite summaries and an authenticated embedded dashboard with live sessions, 30-day history, deletion, purge, and automatic browser launch.

**Architecture:** Keep `SessionRegistry` authoritative for live transitions. Add a `SessionCoordinator` that applies events, performs best-effort persistence through a narrow SQLite repository, updates aggregates, and publishes the latest active-session list through a watch channel. Serve embedded same-origin assets and SSE from the existing Axum service; browser clients exchange one-time launch tokens for memory-only cookies.

**Tech Stack:** Rust 2024, Tokio, Axum, rusqlite with bundled SQLite, tower-http cookies/headers where useful, serde, UUID, Server-Sent Events, embedded HTML/CSS/JavaScript, webbrowser.

---

## File Map

- Modify `Cargo.toml`: add SQLite, cookie, stream, and browser-launch dependencies.
- Modify `src/paths.rs`: add the per-user data directory and database path.
- Create `src/storage/mod.rs`: storage facade and live-only fallback type.
- Create `src/storage/model.rs`: closed persisted summary and aggregate response types.
- Create `src/storage/sqlite.rs`: migrations and all parameterized SQLite operations.
- Create `src/service/coordinator.rs`: registry, persistence, aggregation, and live publication ordering.
- Create `src/service/dashboard_auth.rs`: one-time launch tokens and browser sessions.
- Modify `src/service/api.rs`: dashboard assets, history APIs, deletion, purge, and SSE.
- Modify `src/service/runtime.rs`: initialize storage, recovery, retention, coordinator, and dashboard-client liveness.
- Modify `src/service/mod.rs`: export new service components.
- Create `src/dashboard/mod.rs`: embedded assets and response helpers.
- Create `src/dashboard/index.html`: local dashboard shell.
- Create `src/dashboard/app.css`: responsive accessible presentation.
- Create `src/dashboard/app.js`: live rendering, history loading, deletion, purge, and reconnect behavior.
- Modify `src/cli.rs`: add `dashboard` command.
- Modify `src/client.rs`: request a dashboard launch URL.
- Modify `src/app.rs`: ensure service and open the system browser.
- Modify `src/lib.rs`: export storage/dashboard modules.
- Create `tests/storage.rs`: migration, recovery, retention, deletion, purge, aggregate, and privacy tests.
- Create `tests/dashboard_api.rs`: launch auth, cookie origin checks, history, and SSE tests.
- Create `tests/dashboard_command.rs`: CLI launch behavior and fallback output.
- Modify `tests/privacy_contract.rs`: assert persisted and dashboard surfaces exclude prohibited data.
- Modify `README.md`: document Phase 2 commands, storage, retention, privacy, and local verification.

## Task 1: Add Storage Types, Paths, And Dependencies

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/lib.rs`
- Modify: `src/paths.rs`
- Create: `src/storage/mod.rs`
- Create: `src/storage/model.rs`
- Test: `tests/storage.rs`

- [ ] **Step 1: Write failing path and serialization tests**

Add tests proving `AppPaths::database_file()` is under the data directory and that `SessionSummary` JSON exposes only the approved closed field set.

```rust
#[test]
fn summary_json_contains_only_approved_fields() {
    let value = serde_json::to_value(sample_summary()).unwrap();
    let keys: BTreeSet<_> = value.as_object().unwrap().keys().cloned().collect();
    assert_eq!(keys, BTreeSet::from([
        "adapter_id".into(), "adapter_kind".into(), "agent_id".into(),
        "avg_cpu_percent".into(), "avg_memory_bytes".into(),
        "context_final".into(), "context_peak".into(), "duration_ms".into(),
        "ended_at_ms".into(), "exit_code".into(), "failure_category".into(),
        "final_state".into(), "peak_cpu_percent".into(), "peak_memory_bytes".into(),
        "renderer_id".into(), "session_id".into(), "started_at_ms".into(),
        "tool_event_count".into(), "transition_count".into(),
    ]));
}
```

- [ ] **Step 2: Run the focused tests and confirm failure**

Run: `cargo test --locked --test storage -- --nocapture`

Expected: compilation fails because storage types and the database path do not exist.

- [ ] **Step 3: Add minimal dependencies and closed models**

Add:

```toml
async-stream = "0.3"
cookie = "0.18"
futures-util = "0.3"
rusqlite = { version = "0.38", features = ["bundled"] }
webbrowser = "1"
```

Define `AdapterKind`, `SessionSummary`, `HistoryStats`, and `HistoryPage` with `serde(deny_unknown_fields)` where deserialization is used. Do not add generic metadata or JSON value fields.

Extend `AppPaths`:

```rust
pub struct AppPaths {
    pub config_dir: PathBuf,
    pub runtime_dir: PathBuf,
    pub data_dir: PathBuf,
}

pub fn database_file(&self) -> PathBuf {
    self.data_dir.join("sessions.sqlite3")
}
```

- [ ] **Step 4: Run focused tests**

Run: `cargo test --locked --test storage summary_json_contains_only_approved_fields`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock src/lib.rs src/paths.rs src/storage/mod.rs src/storage/model.rs tests/storage.rs
git commit -m "feat: define phase 2 storage model"
```

## Task 2: Implement SQLite Migrations And Repository Operations

**Files:**
- Create: `src/storage/sqlite.rs`
- Modify: `src/storage/mod.rs`
- Modify: `tests/storage.rs`

- [ ] **Step 1: Write failing migration and CRUD tests**

Cover:

- Fresh database creates schema version 1.
- Reopening migration is idempotent.
- Started header can be finalized and listed newest first.
- Parameter values containing quotes remain data, not SQL.
- Active rows cannot be deleted.
- Finalized row deletion and full purge work.

Use temporary files and public repository methods rather than direct SQL except for schema privacy inspection.

- [ ] **Step 2: Run tests and confirm failure**

Run: `cargo test --locked --test storage sqlite_ -- --nocapture`

Expected: FAIL because `SqliteStore` does not exist.

- [ ] **Step 3: Implement schema and operations**

Create a schema with explicit columns only:

```sql
CREATE TABLE sessions (
  session_id TEXT PRIMARY KEY NOT NULL,
  agent_id TEXT NOT NULL,
  adapter_id TEXT NOT NULL,
  renderer_id TEXT,
  adapter_kind TEXT NOT NULL,
  started_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL,
  ended_at_ms INTEGER,
  duration_ms INTEGER,
  final_state TEXT NOT NULL,
  failure_category TEXT,
  exit_code INTEGER,
  transition_count INTEGER NOT NULL DEFAULT 0,
  tool_event_count INTEGER NOT NULL DEFAULT 0,
  cpu_sample_count INTEGER NOT NULL DEFAULT 0,
  cpu_sum REAL NOT NULL DEFAULT 0,
  peak_cpu_percent REAL,
  memory_sample_count INTEGER NOT NULL DEFAULT 0,
  memory_sum INTEGER NOT NULL DEFAULT 0,
  peak_memory_bytes INTEGER,
  context_peak REAL,
  context_final REAL,
  context_quality TEXT,
  context_source TEXT,
  finalized INTEGER NOT NULL DEFAULT 0 CHECK(finalized IN (0, 1))
);
CREATE INDEX sessions_history_idx
ON sessions(finalized, ended_at_ms DESC);
```

Implement all statements with `params![]`. Set `PRAGMA journal_mode = WAL`, `foreign_keys = ON`, and a short busy timeout. Run migrations in an immediate transaction and update `PRAGMA user_version = 1` only after success.

- [ ] **Step 4: Verify CRUD tests**

Run: `cargo test --locked --test storage sqlite_ -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/storage/mod.rs src/storage/sqlite.rs tests/storage.rs
git commit -m "feat: persist privacy safe session summaries"
```

## Task 3: Add Recovery, Retention, Aggregation, And Live-Only Fallback

**Files:**
- Modify: `src/storage/mod.rs`
- Modify: `src/storage/sqlite.rs`
- Modify: `tests/storage.rs`

- [ ] **Step 1: Write failing behavior tests**

Add tests proving:

- Unfinished rows become `interrupted` on reopen.
- Recovery uses `updated_at_ms` as end time and non-negative duration.
- Cleanup deletes finalized rows strictly older than the cutoff.
- Cleanup preserves active rows and boundary-equal finalized rows.
- Statistics count terminal states and calculate duration/resource aggregates.
- Unsupported `user_version` returns an initialization error.

- [ ] **Step 2: Run tests and confirm failure**

Run: `cargo test --locked --test storage recovery_ retention_ stats_ -- --nocapture`

Expected: FAIL because the operations are not implemented.

- [ ] **Step 3: Implement recovery and retention**

Add repository methods:

```rust
pub fn recover_interrupted(&self) -> Result<usize>;
pub fn cleanup_before(&self, cutoff_ms: u64) -> Result<usize>;
pub fn history(&self, limit: u32, offset: u32) -> Result<HistoryPage>;
pub fn stats(&self) -> Result<HistoryStats>;
pub fn delete_finalized(&self, session_id: Uuid) -> Result<DeleteOutcome>;
pub fn purge_finalized(&self) -> Result<usize>;
```

Wrap the concrete store in:

```rust
#[derive(Clone)]
pub enum HistoryStore {
    Available(Arc<SqliteStore>),
    Unavailable(Arc<str>),
}
```

The unavailable variant returns a stable `HistoryUnavailable` error while callers can continue live operation.

- [ ] **Step 4: Run the full storage suite**

Run: `cargo test --locked --test storage -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/storage/mod.rs src/storage/sqlite.rs tests/storage.rs
git commit -m "feat: add history recovery and retention"
```

## Task 4: Introduce The Session Coordinator And Live Publication

**Files:**
- Create: `src/service/coordinator.rs`
- Modify: `src/service/registry.rs`
- Modify: `src/service/mod.rs`
- Modify: `src/service/runtime.rs`
- Modify: `src/service/api.rs`
- Modify: `tests/service_api.rs`
- Create: `tests/coordinator.rs`

- [ ] **Step 1: Write failing coordinator tests**

Test that:

- Successful registry transitions are persisted and published.
- Invalid transitions are neither persisted nor published.
- Persistence failure still returns the valid live snapshot.
- Completed sessions disappear from the active list after the terminal snapshot is published.
- Metrics update storage aggregates without changing lifecycle state.

- [ ] **Step 2: Run tests and confirm failure**

Run: `cargo test --locked --test coordinator -- --nocapture`

Expected: compilation fails because `SessionCoordinator` does not exist.

- [ ] **Step 3: Implement coordinator ordering**

Use:

```rust
pub struct SessionCoordinator {
    registry: SessionRegistry,
    history: HistoryStore,
    live_tx: watch::Sender<Vec<SessionSnapshot>>,
}

pub async fn apply(&self, event: Event) -> Result<SessionSnapshot, RegistryError> {
    let kind = event.kind.clone();
    let snapshot = self.registry.apply(event).await?;
    let _ = self.history.record(&snapshot, &kind).await;
    let active = self.registry.active_sessions().await;
    self.live_tx.send_replace(active);
    Ok(snapshot)
}
```

Add `active_sessions()` sorted by `started_at_ms` then UUID for deterministic responses. Keep persistence best effort and metadata-only.

- [ ] **Step 4: Route existing event ingestion through coordinator**

Replace `ApiState.registry` with `ApiState.coordinator`. Preserve the existing `/v1/events` response and validation behavior.

- [ ] **Step 5: Run coordinator and Phase 1 service tests**

Run: `cargo test --locked --test coordinator --test service_api --test service_startup -- --test-threads=1`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/service/coordinator.rs src/service/registry.rs src/service/mod.rs src/service/runtime.rs src/service/api.rs tests/coordinator.rs tests/service_api.rs
git commit -m "feat: coordinate live and persisted sessions"
```

## Task 5: Add Browser Launch Tokens And Session Cookies

**Files:**
- Create: `src/service/dashboard_auth.rs`
- Modify: `src/service/mod.rs`
- Modify: `src/service/api.rs`
- Create: `tests/dashboard_api.rs`

- [ ] **Step 1: Write failing authentication tests**

Cover:

- Bearer-authenticated launch request returns a loopback launch URL.
- Launch token expires after 60 seconds.
- Launch token succeeds once and fails on reuse.
- Successful launch redirects without the token and sets `HttpOnly; SameSite=Strict` cookie.
- Browser cookie authenticates reads.
- Cookie-authenticated deletion without same-origin `Origin` is rejected with `403`.
- The bearer token remains valid for CLI clients without `Origin`.

- [ ] **Step 2: Run tests and confirm failure**

Run: `cargo test --locked --test dashboard_api launch_ cookie_ -- --nocapture`

Expected: FAIL because dashboard auth routes do not exist.

- [ ] **Step 3: Implement in-memory auth state**

Define:

```rust
pub struct DashboardAuth {
    launch_tokens: Arc<Mutex<HashMap<String, Instant>>>,
    sessions: Arc<Mutex<HashSet<String>>>,
}
```

Generate tokens from two UUID v4 values, consume them atomically, remove expired entries on each mutation, and never serialize the long-lived bearer token into HTML or JavaScript.

Use a cookie named `smt_dashboard` with `HttpOnly`, `SameSite=Strict`, `Path=/`, and no `Secure` flag because the service is intentionally loopback HTTP. Browser sessions remain memory-only.

- [ ] **Step 4: Run focused auth tests**

Run: `cargo test --locked --test dashboard_api launch_ cookie_ -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/service/dashboard_auth.rs src/service/mod.rs src/service/api.rs tests/dashboard_api.rs
git commit -m "feat: authenticate local dashboard sessions"
```

## Task 6: Add History APIs, SSE, And Dashboard Client Liveness

**Files:**
- Modify: `src/service/api.rs`
- Modify: `src/service/runtime.rs`
- Modify: `src/service/coordinator.rs`
- Modify: `tests/dashboard_api.rs`
- Modify: `tests/service_api.rs`

- [ ] **Step 1: Write failing API tests**

Cover:

- `GET /v1/sessions/active` returns only active snapshots.
- `GET /v1/history?limit=50&offset=0` returns bounded newest-first summaries.
- Limit `0` or over `100` returns `400`.
- History unavailable returns `503` without breaking active sessions.
- Delete missing returns `404`; active returns `409`; finalized returns `204`.
- Purge removes only finalized rows.
- SSE immediately emits a `sessions` event and emits another after a transition.
- A connected SSE client prevents idle shutdown; dropping it releases the service.

- [ ] **Step 2: Run tests and confirm failure**

Run: `cargo test --locked --test dashboard_api --test service_api -- --test-threads=1`

Expected: FAIL on missing routes and client-liveness behavior.

- [ ] **Step 3: Implement authenticated endpoints**

Use closed response structs. Map storage errors to:

```json
{"code":"history_unavailable","message":"session history is unavailable"}
```

Do not return raw SQLite messages to browser clients.

- [ ] **Step 4: Implement SSE**

Build the stream from `watch::Receiver<Vec<SessionSnapshot>>` and a heartbeat interval. Increment a shared dashboard-client counter when the handler starts and decrement it with a drop guard when the stream closes.

Update idle shutdown to require both:

```rust
coordinator.idle_for().await >= idle_timeout
    && dashboard_clients.load(Ordering::Relaxed) == 0
```

- [ ] **Step 5: Run API and lifetime tests**

Run: `cargo test --locked --test dashboard_api --test service_api -- --test-threads=1`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/service/api.rs src/service/runtime.rs src/service/coordinator.rs tests/dashboard_api.rs tests/service_api.rs
git commit -m "feat: stream live sessions and history"
```

## Task 7: Build And Embed The Dashboard

**Files:**
- Create: `src/dashboard/mod.rs`
- Create: `src/dashboard/index.html`
- Create: `src/dashboard/app.css`
- Create: `src/dashboard/app.js`
- Modify: `src/lib.rs`
- Modify: `src/service/api.rs`
- Modify: `tests/dashboard_api.rs`

- [ ] **Step 1: Write failing asset and security tests**

Test that:

- `/dashboard` serves HTML after browser authentication.
- Assets have correct content types and `nosniff`.
- CSP allows only same-origin script/style/connect sources.
- HTML contains no `http://`, `https://`, bearer token, or inline external asset reference.
- JavaScript contains no prohibited protocol field names.

- [ ] **Step 2: Run tests and confirm failure**

Run: `cargo test --locked --test dashboard_api assets_ -- --nocapture`

Expected: FAIL because embedded assets do not exist.

- [ ] **Step 3: Implement embedded assets**

Expose constants through `include_str!` and serve them with static ETags based on package version. Keep JavaScript dependency-free.

The HTML contains:

- A header with connection/storage status.
- Live and History tab buttons.
- Live-session card container.
- History summary cards and table.
- Accessible confirmation dialog for delete/purge.
- Status region with `aria-live="polite"`.

- [ ] **Step 4: Implement dashboard behavior**

JavaScript must:

- Open `EventSource('/v1/sessions/stream')` with cookie credentials.
- Render duration locally from timestamps.
- Render unavailable metrics as `Unavailable`, preserving quality/source labels.
- Fetch history and stats when History opens.
- Send same-origin DELETE requests for row deletion and purge.
- Reconnect SSE with bounded 1s, 2s, 5s, 10s delay and reset after success.
- Never use `localStorage`, analytics, service workers, or outbound fetches.

- [ ] **Step 5: Run dashboard asset tests**

Run: `cargo test --locked --test dashboard_api assets_ -- --nocapture`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/dashboard src/lib.rs src/service/api.rs tests/dashboard_api.rs
git commit -m "feat: embed local session dashboard"
```

## Task 8: Add `smt dashboard` And Automatic Browser Launch

**Files:**
- Modify: `src/cli.rs`
- Modify: `src/client.rs`
- Modify: `src/app.rs`
- Modify: `src/service/api.rs`
- Create: `tests/dashboard_command.rs`
- Modify: `tests/cli_help.rs`

- [ ] **Step 1: Write failing CLI tests**

Cover:

- Help lists `dashboard`.
- Dashboard ensures a missing service starts.
- Dashboard requests a one-time launch URL.
- Browser opener receives the launch URL.
- Browser-open failure prints the same usable URL and exits nonzero.

Inject browser opening behind:

```rust
pub trait BrowserOpener {
    fn open(&self, url: &str) -> anyhow::Result<()>;
}
```

Production uses `webbrowser::open`; tests use a recording fake.

- [ ] **Step 2: Run tests and confirm failure**

Run: `cargo test --locked --test dashboard_command --test cli_help -- --nocapture`

Expected: FAIL because the command does not exist.

- [ ] **Step 3: Implement client and command**

Add:

```rust
pub async fn dashboard_launch_url(&self) -> Result<String>;
```

The command calls `ServiceClient::ensure`, requests the URL, and opens it automatically. On open failure:

```text
could not open browser; open this local URL manually:
http://127.0.0.1:<port>/dashboard/launch?token=<short-lived-token>
```

- [ ] **Step 4: Run focused CLI tests**

Run: `cargo test --locked --test dashboard_command --test cli_help -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/cli.rs src/client.rs src/app.rs src/service/api.rs tests/dashboard_command.rs tests/cli_help.rs
git commit -m "feat: open authenticated local dashboard"
```

## Task 9: Harden Privacy, Failure Isolation, Documentation, And Verification

**Files:**
- Modify: `tests/privacy_contract.rs`
- Modify: `tests/storage.rs`
- Modify: `tests/dashboard_api.rs`
- Modify: `README.md`
- Modify: `.gitignore` if SQLite development artifacts need coverage

- [ ] **Step 1: Add end-to-end privacy tests**

Insert representative secret strings through every permitted identifier validation boundary and verify they are rejected or absent from:

- SQLite schema and rows.
- History JSON.
- SSE payloads.
- Embedded assets.
- Error responses.

Assert no active `.github/workflows` file is added.

- [ ] **Step 2: Add live-only degradation test**

Start the service with an invalid database path, send valid lifecycle events, consume active-session SSE, and verify history returns `503` while the wrapper/service remains usable.

- [ ] **Step 3: Update README**

Document:

- `smt dashboard` automatic browser behavior.
- Local SQLite location through platform data directories.
- 30-day retention, delete, and purge.
- Short-lived browser launch authentication.
- No outbound requests and no GitHub Actions workflow.
- Local verification commands.

- [ ] **Step 4: Run formatting and focused checks**

Run:

```bash
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features -- --test-threads=1
cargo build --locked --release
git diff --check
```

Expected: all commands exit `0` and all tests pass.

- [ ] **Step 5: Run privacy and runtime smoke checks**

Run:

```bash
SMT_TEST_FORCE_SERVICE_FAILURE=1 target/release/smt run --no-status -- sh -c 'printf secret-output'
find .github/workflows -type f -print 2>/dev/null
```

Expected: stdout is exactly `secret-output`; no active workflow files are printed.

- [ ] **Step 6: Review the aggregate diff**

Check:

- No prohibited data field or generic metadata blob was introduced.
- Every SQLite statement is parameterized.
- Dashboard assets make no external requests.
- Storage errors cannot stop live sessions or wrapped commands.
- Only Phase 2 scope changed.

- [ ] **Step 7: Commit**

```bash
git add README.md .gitignore tests/privacy_contract.rs tests/storage.rs tests/dashboard_api.rs
git commit -m "docs: verify phase 2 privacy and dashboard"
```

## Final Branch Gate

- [ ] Run the complete verification suite fresh.
- [ ] Confirm `git status -sb` is clean.
- [ ] Confirm `git diff main...HEAD --check` is clean.
- [ ] Confirm `.github/workflows` contains no active workflow.
- [ ] Review the full feature diff for security, lifecycle, retention, and cross-platform assumptions.
- [ ] Push the feature branch, create a ready PR, and merge only after local verification. Do not run or add GitHub Actions.
