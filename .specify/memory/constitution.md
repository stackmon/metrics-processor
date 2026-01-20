<!--
Sync Impact Report - Version 1.0.0 (Initial Constitution)
══════════════════════════════════════════════════════════

Version Change: N/A → 1.0.0 (Initial ratification)

Principles Established:
  • I. Code Quality Standards (NEW)
  • II. Testing Excellence (NEW)
  • III. User Experience Consistency (NEW)
  • IV. Performance Requirements (NEW)

Templates Requiring Updates:
  ✅ plan-template.md - Reviewed: Constitution Check section aligns
  ✅ spec-template.md - Reviewed: Requirements sections align
  ✅ tasks-template.md - Reviewed: Test-first approach aligns
  ✅ No agent command files found requiring updates

Follow-up Items:
  • None - All placeholders resolved

Notes:
  • Initial constitution ratified for cloudmon-metrics project
  • Rust-specific practices incorporated based on project analysis
  • All principles aligned with metrics-processor domain (real-time processing)
-->

# CloudMon Metrics Processor Constitution

## Core Principles

### I. Code Quality Standards

All code contributed to cloudmon-metrics MUST adhere to the following non-negotiable quality standards:

- **Rust Idiomatic Practices**: Code MUST follow Rust idioms and the official Rust API guidelines. Use `clippy` with default lints and address all warnings before merge.
- **Documentation**: All public APIs MUST have comprehensive rustdoc comments with examples. Module-level documentation MUST explain purpose and usage patterns.
- **Type Safety**: Leverage Rust's type system to prevent invalid states. Prefer compile-time guarantees over runtime checks where feasible.
- **Error Handling**: All errors MUST be properly typed using custom error enums or the `thiserror` crate. Avoid `unwrap()` and `expect()` in production code paths; use `?` operator and explicit error propagation.
- **Code Review**: All changes MUST pass peer review verifying adherence to these standards before merge.

**Rationale**: As a metrics processing system handling real-time data conversion and health reporting, code quality directly impacts reliability, maintainability, and operator confidence. Rust's strong typing and explicit error handling enable us to catch issues at compile time rather than in production.

### II. Testing Excellence

Testing is mandatory and MUST cover multiple levels to ensure system reliability:

- **Unit Test Coverage**: All business logic functions and data transformations MUST have unit tests. Target minimum 95% code coverage for core base.
- **Integration Tests**: API endpoints and TSDB query integrations MUST have integration tests exercising real HTTP workflows and data transformations end-to-end.
- **Contract Testing**: When interfaces change (config schema, API contracts, TSDB query formats), contract tests MUST be added to verify backward compatibility or explicitly document breaking changes.
- **Mock External Dependencies**: Use `mockito` for external service mocking (TSDB backends, status dashboards). Never depend on live external services in automated tests.
- **Test Organization**: Tests MUST be organized as `#[cfg(test)]` modules within implementation files for unit tests, and in `tests/` directory for integration tests following Rust conventions.

**Rationale**: Metrics processing involves complex data transformations (raw metrics → semaphore values) and integrations with external time-series databases. Comprehensive testing prevents regression in calculation logic and ensures accurate health reporting to dashboards.

### III. User Experience Consistency

To ensure predictable and intuitive behavior across all interaction points:

- **API Consistency**: All HTTP API endpoints MUST follow RESTful conventions. Response formats MUST be consistent (use `serde_json` for JSON serialization with consistent error response schemas).
- **Configuration Interface**: Configuration files MUST use YAML format with clear schema validation. All configuration errors MUST provide actionable error messages indicating the exact field and expected format.
- **Logging Standards**: Use structured logging via `tracing` crate. All log entries MUST include request IDs (via `tower-http` middleware) for correlation. Log levels MUST be consistently applied: ERROR for actionable issues, WARN for degraded operation, INFO for significant state changes, DEBUG for detailed diagnostics.
- **CLI Consistency**: Binary interfaces (`cloudmon-metrics-convertor`, `cloudmon-metrics-reporter`) MUST provide `--help` output, accept configuration via flags or environment variables, and return appropriate exit codes (0 = success, 1 = error).
- **Documentation Coherence**: User-facing documentation (README, doc/) MUST stay synchronized with code behavior. Breaking changes MUST be documented in a CHANGELOG with migration guides.

**Rationale**: Operators deploying and troubleshooting metrics-processor depend on consistent, predictable behavior. Clear error messages and structured logs reduce time to resolution when investigating health status discrepancies or metric conversion issues.

### IV. Performance Requirements

As a real-time metrics processing system, performance is critical:

- **Response Time Targets**: API endpoints MUST respond within 200ms at p95 under normal load (up to 100 concurrent requests). Metric conversion operations MUST complete within 500ms for datasets up to 1000 data points.
- **Resource Efficiency**: Binary memory footprint MUST remain under 100MB RSS under normal operation. Avoid unnecessary allocations in hot paths; prefer stack allocation and borrowing.
- **Async Operations**: All I/O operations (HTTP requests to TSDB, status dashboard updates) MUST use async/await with `tokio` runtime. Never block the async runtime with synchronous I/O or CPU-intensive work.
- **Query Optimization**: TSDB queries MUST be batched where possible. Implement query result caching with appropriate TTLs (configurable, default 60s) to reduce backend load.
- **Performance Testing**: Performance-critical paths (metric evaluation expressions, query construction) MUST have benchmark tests using `cargo bench` or criterion. Regressions >10% MUST be investigated before merge.

**Rationale**: Metrics-processor operates in a real-time monitoring pipeline where delayed health status updates can mask actual service outages. Performance requirements ensure timely visibility into system health, which is the core value proposition.

## Development Workflow

### Code Contribution Process

1. **Branch Strategy**: Use feature branches named `feature/<issue-id>-<description>` or `fix/<issue-id>-<description>`.
2. **Pre-Commit Checks**: All commits MUST pass `cargo fmt`, `cargo clippy`, and pre-commit hooks (`.pre-commit-config.yaml`).
3. **CI/CD Gates**: All PRs MUST pass:
   - Compilation (`cargo build`)
   - Linting (`cargo clippy`)
   - Tests (`cargo test`)
   - Docker build validation (Dockerfile)
4. **Review Requirements**: At least one approving review from a project maintainer. Reviewers MUST verify Constitution compliance.

### Release Process

- **Versioning**: Follow Semantic Versioning (MAJOR.MINOR.PATCH) in `Cargo.toml`.
- **Changelog**: Update CHANGELOG.md with user-facing changes categorized as Added, Changed, Fixed, Removed.
- **Breaking Changes**: MAJOR version bumps MUST include migration documentation and deprecation warnings in at least one MINOR release before removal.

## Governance

### Constitution Authority

This Constitution supersedes all other development practices and guidelines. All pull requests, code reviews, and design decisions MUST align with these principles. When conflicts arise between convenience and Constitutional principles, principles take precedence.

### Amendment Process

Amendments to this Constitution require:

1. **Documentation**: Proposed change documented with rationale and impact analysis.
2. **Review**: Discussion and approval by at least two project maintainers.
3. **Migration Plan**: If amendment affects existing code, a migration plan and timeline MUST be provided.
4. **Version Update**: Constitution version MUST be incremented following semantic versioning.

### Compliance & Enforcement

- All code reviews MUST explicitly verify Constitutional compliance in the review checklist.
- Complexity that violates principles (e.g., skipping tests for "quick fixes") MUST be justified in writing and tracked as technical debt with remediation plans.
- This Constitution applies to all code within the `cloudmon-metrics` project regardless of contributor.

### Living Document

This Constitution is maintained at `.specify/memory/constitution.md`. Runtime development guidance and implementation details are maintained separately in project documentation (`README.md`, `doc/`).

**Version**: 1.0.0 | **Ratified**: 2026-01-20 | **Last Amended**: 2026-01-20
