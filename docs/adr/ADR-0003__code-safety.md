# ADR-0003: Code Safety & Explicit Truth

* **Status:** Accepted
* **Date:** 2025-12-22
* **Context:** The codebase must transition from implicit stability ("it works") to explicit, verifiable stability ("it is proven correct by rules").

## Context

Implicit assumptions about code safety (e.g., "we only unwrap in tests") are fragile. As the team and codebase grow, these unwritten rules degrade, leading to "truth decay" and reliability issues.

We need to codify our safety standards to ensure the system remains robust and predictable.

## Decision

We adopt the following explicit norms for Rust code in `hauski`.

### 1. `unwrap()` and `expect()`

* **Production Code (Runtime):**
  * **Forbidden:** `unwrap()` is strictly forbidden in runtime logic (handlers, background tasks, core loops).
  * **Restricted:** `expect()` is allowed ONLY if the invariant is locally provable and documented (e.g., locking a mutex that we know isn't poisoned, though `poisoned.into_inner()` is preferred).
  * **Preferred:** Use `?` (error propagation) or `match` to handle errors gracefully.

* **Startup Phase 1 (Config/Env Parsing):**
  * **Allowed:** `unwrap()` or `expect()` are acceptable **only** during the synchronous initialization phase (reading environment variables, parsing config files) **before** the async runtime/server loop is started.
  * **Rationale:** If basic configuration is invalid, the process *should* crash immediately and loudly to prevent running in an undefined state.

* **Tests:**
  * **Allowed:** `unwrap()` is idiomatic in tests to assert success.

### 2. Error Handling

* **Explicit Types:** Public APIs must return `Result<T, E>`, where `E` is a specific error type (e.g., `thiserror` enum) or `anyhow::Result` for top-level applications.
* **No Silent Failures:** Errors must be logged (via `tracing`) or returned. Swallowing errors (e.g., `let _ = ...`) is forbidden unless explicitly commented with a reason (e.g., "fire-and-forget metrics").

### 3. Panics

* **No Panics in Handlers:** HTTP handlers and core logic loops must never panic. A panic in a thread/task poisons the system state.
* **Recovery:** If a panic is theoretically possible (e.g., in external C-FFI), it must be wrapped in `std::panic::catch_unwind` or run in an isolated process/actor.

## Consequences

* **Positive:**
  * Code reviews become objective ("Rule 1 violation") rather than subjective ("I don't like this").
  * Reliability improves; unexpected crashes decrease.
* **Negative:**
  * Code verbosity increases slightly due to explicit error handling.
  * Prototyping might feel slower (requires handling `Result`s immediately).

## Compliance

This ADR is enforced by:
1.  **Code Review:** Reviewers must block PRs violating these rules.
2.  **Linter (Future):** We will configure `clippy` to deny `unwrap` in specific scopes (e.g., `clippy::unwrap_used`).
