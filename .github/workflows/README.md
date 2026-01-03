# CI Workflows

> **Note:** This document is a conceptual map, not an exhaustive list. Please refer to the file listing for the canonical set of workflows.
>
> **Source of Truth:** The workflow files themselves are the authoritative source. Each key workflow file is marked with a `# CLASS: <type>` header.

To prevent drift and redundancy, workflows are semantically classified into three categories.

## 1. Baseline (Always)
**Must always run.** Minimal, fast, unambiguous.
*   `ci.yml`: The central CI pipeline. Covers Build, Fast Tests, and Linting. This is the **Primary Source of Truth** for the mergeability of the PR.
    *   *Note:* While it includes a conditional `security` job (for `full-ci`), comprehensive security checks are managed by `security.yml`.
*   `wgx-guard.yml`: Ensures the integrity and configuration of the `wgx` tool.
*   `ci-tools.yml`: Ensures internal CI tools are built correctly.

## 2. Deepening (Contextual)
**Runs only on specific paths, labels, or schedules.** Adds depth to the baseline.
*   `heavy.yml`: E2E tests and release builds. Triggered via `full-ci` label.
*   `coverage.yml`: Code coverage analysis.
*   `security.yml`: Comprehensive security scans (`cargo-deny`, `cargo-audit`) on schedule or specific file changes.
*   `links.yml`: Dedicated, potentially more exhaustive link checking (Lychee).
*   `wgx-smoke.yml`: Functional smoke tests.
*   `metrics.yml`: Generation of metric snapshots.
*   `playbook-gate.yml`: Validation of playbooks.
*   `vendor.yml` / `reusable-validate-vendor.yml`: Checks for vendored dependencies.

## 3. Meta / Governance
**Policy, Contracts, Security, and Processes.**
*   `contracts.yml`, `contracts-validate.yml`: Verification of architectural contracts (`docs/contracts`).
*   `policy-ci.yml`: Validation of policy definitions.
*   `ai-context-guard.yml`: Protection of the `.ai-context.yml` file.
*   `validate-*.yml`: Schema validation (Events, Dev-Tooling).
*   `secret-scan-gitleaks.yml`: Secret scanning (runs on PRs and pushes).
*   `release.yml`: Release automation.
*   `pr-heimgewebe-commands.yml`: ChatOps and PR interactions.
*   `codex-review.yml`: Automated code review (runs only if `OPENAI_API_KEY` is set).
*   `review-cycle-check.yml`: Review process logic.

---

### Principles
*   **Baseline** is the authority for "Mergeable".
*   **Deepening** provides additional confidence or specialized metrics but does not replace the Baseline.
*   **Meta** ensures compliance with the ecosystem (Heimgewebe) and project policies.
