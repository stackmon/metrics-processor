# Implementation Plan: Status Dashboard API V2 Migration

**Branch**: `003-sd-api-v2-migration` | **Date**: 2025-01-23 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/003-sd-api-v2-migration/spec.md`

## Summary

Migrate the `cloudmon-metrics-reporter` from Status Dashboard API V1 (`/v1/component_status`) to V2 (`/v2/incidents`, `/v2/components`). The migration introduces component ID caching with retry logic, restructures incident data with static title/description for security, and increases HTTP timeout from 2s to 10s. Authorization mechanism (HMAC-JWT) remains unchanged. All 17 functional requirements (FR-001 through FR-017) are addressed through a nested HashMap cache structure with subset attribute matching, structured diagnostic logging separate from API payloads, and graceful error handling with automatic recovery.

## Technical Context

**Language/Version**: Rust 2021 edition (Cargo.toml: edition = "2021", likely Rust 1.70+)  
**Primary Dependencies**: 
- `reqwest ~0.11` (HTTP client with rustls-tls, json features)
- `chrono ~0.4` (datetime handling, RFC3339 formatting)
- `serde ~1.0` + `serde_json ~1.0` (JSON serialization)
- `tokio ~1.42` (async runtime with full features)
- `tracing ~0.1` + `tracing-subscriber ~0.3` (structured logging)
- `jwt ~0.16`, `hmac ~0.12`, `sha2 ~0.10` (HMAC-JWT authentication)
- **NEW**: `anyhow ~1.0` (for Result<T> error handling in cache functions)

**Storage**: In-memory HashMap for component ID cache (~100 components × 200 bytes = ~20KB); no persistent storage  

**Testing**: 
- `cargo test` with `#[cfg(test)]` unit tests in source files
- Integration tests in `tests/` directory using `mockito ~1.0`, `tokio-test`, `tower` utilities
- Existing test files: `tests/integration_api.rs`, `tests/integration_health.rs`, `tests/documentation_validation.rs`

**Target Platform**: Linux server (primary), macOS (development); binary target `cloudmon-metrics-reporter` from `src/bin/reporter.rs`

**Project Type**: Single Rust project (library + 2 binaries: `cloudmon-metrics-convertor`, `cloudmon-metrics-reporter`)

**Performance Goals**: 
- API response time: <200ms p95 under normal load (100 concurrent requests per Constitution IV)
- Metric conversion: <500ms for datasets up to 1000 data points
- **Reporter-specific**: HTTP timeout 10s (increased from 2s per FR-014), monitoring cycle ~60s

**Constraints**: 
- Memory footprint: <100MB RSS under normal operation (Constitution IV)
- Component cache refresh: 3 retries × 60s delays on startup (FR-006)
- Incident creation: no immediate retry on failure, rely on 60s monitoring cycle (FR-015)
- HTTP timeout: 10 seconds for V2 API calls (FR-014)

**Scale/Scope**: 
- ~100 components in Status Dashboard
- ~10-20 monitored services per environment
- ~1-10 incidents/minute under normal load
- ~5000 lines of code in reporter binary (incremental change to existing)

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

### Principle I: Code Quality Standards

| Requirement | Status | Compliance Notes |
|-------------|--------|------------------|
| Rust idiomatic practices | ✅ PASS | Uses standard collections (HashMap), serde for serialization, async/await patterns |
| Documentation | ✅ PASS | Existing `reporter.rs` has doc comments; new functions will follow rustdoc conventions |
| Type safety | ✅ PASS | Strong typing for all structs (ComponentAttribute, IncidentData); uses Result<T> for errors |
| Error handling | ✅ PASS | Adding `anyhow::Result<T>` for cache functions; no `unwrap()` in production paths (only in main setup) |
| Code review | ✅ PASS | Feature branch `003-sd-api-v2-migration` will undergo peer review before merge |

### Principle II: Testing Excellence

| Requirement | Status | Compliance Notes |
|-------------|--------|------------------|
| Unit test coverage | ✅ PASS | Plan includes unit tests for cache building, component matching, timestamp handling (target 95% per Constitution) |
| Integration tests | ✅ PASS | Using `mockito` to mock `/v2/components` and `/v2/incidents` endpoints; testing full flow |
| Contract testing | ✅ PASS | OpenAPI schema validation against `openapi.yaml`; contracts defined in `specs/003-sd-api-v2-migration/contracts/` |
| Mock external dependencies | ✅ PASS | `mockito ~1.0` already in dev-dependencies for mocking Status Dashboard API |
| Test organization | ✅ PASS | Unit tests in `#[cfg(test)]` modules; integration tests in `tests/reporter_v2_integration.rs` (new file) |

### Principle III: User Experience Consistency

| Requirement | Status | Compliance Notes |
|-------------|--------|------------------|
| API consistency | ✅ PASS | Reporter uses `serde_json` for consistent JSON handling; follows existing patterns |
| Configuration interface | ✅ PASS | No config changes required (FR-008); existing YAML config remains compatible |
| Logging standards | ✅ PASS | Uses `tracing` crate for structured logging; FR-017 adds diagnostic fields (timestamp, service, environment, component details, impact, triggered_metrics) |
| CLI consistency | ✅ PASS | Reporter binary interface unchanged; existing `--help` and exit codes maintained |
| Documentation coherence | ⚠️ DEFER | Will update `doc/` after implementation; migration documented in `specs/003-sd-api-v2-migration/quickstart.md` |

### Principle IV: Performance Requirements

| Requirement | Status | Compliance Notes |
|-------------|--------|------------------|
| Response time targets | ✅ PASS | HTTP timeout increased to 10s (FR-014) accommodates V2 API; monitoring cycle remains 60s |
| Resource efficiency | ✅ PASS | Component cache adds ~20KB memory (negligible); no heap allocation in hot paths |
| Async operations | ✅ PASS | All I/O uses async/await with `tokio` runtime (existing pattern maintained) |
| Query optimization | ✅ PASS | Component cache eliminates repeated API calls; cached lookup is O(n) for ~100 components (acceptable) |
| Performance testing | ⚠️ DEFER | Benchmark tests for cache lookup optional (not in hot path); integration tests cover timeout behavior |

### Development Workflow

| Requirement | Status | Compliance Notes |
|-------------|--------|------------------|
| Branch strategy | ✅ PASS | Feature branch `003-sd-api-v2-migration` already exists |
| Pre-commit checks | ✅ PASS | `.pre-commit-config.yaml` configured; will run `cargo fmt`, `cargo clippy` |
| CI/CD gates | ✅ PASS | Existing Zuul pipeline runs `cargo build`, `cargo clippy`, `cargo test`, Docker build |
| Review requirements | ✅ PASS | PR will require maintainer approval verifying Constitution compliance |

**Overall Assessment**: ✅ **PASS** - All critical gates pass. Two items deferred to post-implementation (documentation updates, optional benchmarks) are acceptable per Constitution governance.

**Re-check After Phase 1**: Will verify test coverage meets 95% target for new code, structured logging includes all FR-017 diagnostic fields.

## Project Structure

### Documentation (this feature)

```text
specs/003-sd-api-v2-migration/
├── spec.md                     # Feature specification (17 FRs, 3 user stories)
├── plan.md                     # This file - implementation plan
├── research.md                 # Phase 0: Technology decisions, API analysis, cache design
├── data-model.md               # Phase 1: Entity definitions, relationships, flows
├── quickstart.md               # Phase 1: Step-by-step implementation guide
├── contracts/                  # Phase 1: API contract specifications
│   ├── README.md               # Contract overview and usage
│   ├── components-api.md       # GET /v2/components endpoint spec
│   ├── incidents-api.md        # POST /v2/incidents endpoint spec
│   ├── request-examples/       # Sample JSON request payloads
│   │   ├── create-incident-single-component.json
│   │   └── create-incident-multi-component.json
│   └── response-examples/      # Sample JSON response payloads
│       ├── components-list.json
│       └── incident-created.json
└── tasks.md                    # Phase 2: Generated by /speckit.tasks (NOT by /speckit.plan)
```

### Source Code (repository root)

```text
src/
├── bin/
│   ├── convertor.rs           # Unchanged - metric conversion binary
│   └── reporter.rs            # ✏️  MODIFIED - V2 API migration implementation
├── api/
│   └── v1.rs                  # Unchanged - health API (ServiceHealthResponse used by reporter)
├── api.rs                     # Unchanged - API router
├── common.rs                  # Unchanged - shared utilities
├── config.rs                  # Unchanged - config parsing (no config changes needed)
├── graphite.rs                # Unchanged - TSDB backend
├── lib.rs                     # Unchanged - library root
└── types.rs                   # Unchanged - core type definitions

tests/
├── integration_api.rs         # Existing - API integration tests
├── integration_health.rs      # Existing - health endpoint tests
├── documentation_validation.rs # Existing - doc validation
└── reporter_v2_integration.rs # ✨ NEW - V2 migration integration tests
    # Tests: component fetching, cache building, incident creation, error handling

Cargo.toml                     # ✏️  MODIFIED - add anyhow ~1.0 dependency
openapi.yaml                   # Reference - Status Dashboard API V2 contract source
```

**Structure Decision**: Single Rust project with library + binaries. This migration only modifies `src/bin/reporter.rs` (adds ~300 lines for cache management and V2 API calls) and adds one new integration test file. No changes to project structure needed - follows existing patterns for binary implementations in `src/bin/` and integration tests in `tests/`.

**Modified Files**:
1. **`Cargo.toml`**: Add `anyhow = "~1.0"` dependency for Result error handling
2. **`src/bin/reporter.rs`**: 
   - Add structs: `StatusDashboardComponent`, `IncidentData`
   - Update `ComponentAttribute` derives (add `PartialOrd`, `Ord`, `Hash`)
   - Add functions: `fetch_components*`, `build_component_id_cache`, `find_component_id`, `build_incident_data`, `create_incident`
   - Update `metric_watcher`: load cache at startup, replace V1 endpoint with V2, add cache miss handling
   - Update `ClientBuilder` timeout from 2s to 10s

**New Files**:
3. **`tests/reporter_v2_integration.rs`**: Integration tests using `mockito` to mock Status Dashboard V2 endpoints

## Complexity Tracking

*No Constitution violations identified. This section intentionally left empty per template instructions.*

**Rationale**: All implementation decisions align with Constitution principles:
- **Simple cache structure**: Standard Rust HashMap (no custom abstractions)
- **Subset matching**: O(n) iteration acceptable for n~100 components
- **Error handling**: `anyhow::Result` is idiomatic Rust pattern
- **No new architectural patterns**: Follows existing reporter structure
- **Testing strategy**: Matches project's existing mockito + tokio-test approach

---

## Phase 0: Research (COMPLETED)

**Status**: ✅ Complete - See [`research.md`](research.md)

**Key Decisions Made**:
1. **Component Cache**: Nested HashMap with sorted attributes for deterministic keys
2. **V2 Incident Payload**: Static title/description, generic content (security per FR-017)
3. **Error Handling**: 3x retry on startup (FR-006), single refresh on miss (FR-005), no immediate retry on incident creation (FR-015)
4. **Testing**: mockito for API mocking, tokio-test for async tests
5. **HTTP Timeout**: 2s → 10s (FR-014)
6. **Authorization**: HMAC-JWT unchanged (FR-008)
7. **Timestamp Handling**: RFC3339 with -1 second adjustment (FR-011)

**All Technical Unknowns Resolved**: No "NEEDS CLARIFICATION" items remain.

---

## Phase 1: Design & Contracts (COMPLETED)

**Status**: ✅ Complete - See design artifacts below

### 1. Data Model
**File**: [`data-model.md`](data-model.md)

**Core Entities Defined**:
- `ComponentAttribute`: Key-value pairs with sorting/hashing support
- `Component` (config): Reporter's view from configuration
- `StatusDashboardComponent` (API): Status Dashboard's view from `/v2/components`
- `ComponentCache`: HashMap mapping (name, attrs) → component_id
- `IncidentData`: V2 incident request payload with static security-compliant fields
- `ServiceHealthResponse`: Existing, unchanged health metric structure

**Key Diagrams**:
- Entity relationship diagram showing data flow
- State transition diagrams for cache and incident creation
- Startup flow: component fetch → cache build → monitoring loop
- Incident creation flow: health query → component lookup → cache refresh on miss → incident POST

### 2. API Contracts
**Directory**: [`contracts/`](contracts/)

**Files Created**:
- `contracts/README.md`: Contract overview and usage guide
- `contracts/components-api.md`: GET /v2/components specification with Rust implementation examples
- `contracts/incidents-api.md`: POST /v2/incidents specification with field constraints (FR-002, FR-017)
- `contracts/request-examples/*.json`: Sample incident creation payloads
- `contracts/response-examples/*.json`: Sample API responses (components list, incident created)

**Validation**: All contracts derived from `/openapi.yaml` (project root, lines 138-270)

### 3. Quickstart Guide
**File**: [`quickstart.md`](quickstart.md)

**Contents**:
- Prerequisites (dependencies, Status Dashboard setup, config compatibility)
- Step-by-step implementation (6 steps: structs, fetch, cache, lookup, incident, metric_watcher update)
- Complete code examples with inline comments
- Unit test suite (cache building, component matching)
- Integration test suite (API mocking with mockito)
- Verification procedures (logs to check, expected outputs)
- Troubleshooting guide (common errors and solutions)

### 4. Agent Context Update
**Status**: ✅ Complete

**Updated File**: `.github/agents/copilot-instructions.md`

**Changes**: Added project-specific context about this feature (database: N/A, project type: single)

---

## Phase 2: Task Generation (DEFERRED)

**Status**: ⏸️  Deferred to `/speckit.tasks` command

**Rationale**: Per plan template instructions, `tasks.md` is generated by a separate command after Phase 1 design is complete. Implementation tasks will be created from:
- Data model entities → implementation tickets
- API contracts → integration test tickets
- Quickstart steps → development workflow tickets

**Next Command**: Run `/speckit.tasks` to generate actionable task breakdown with dependency ordering.

---

## Implementation Summary

### Artifacts Generated

| Phase | Artifact | Status | Lines | Description |
|-------|----------|--------|-------|-------------|
| 0 | `research.md` | ✅ Complete | 308 | Technology decisions, API analysis, cache design rationale |
| 1 | `data-model.md` | ✅ Complete | 650+ | Entity definitions, relationships, data flows, state machines |
| 1 | `contracts/components-api.md` | ✅ Complete | 250+ | GET /v2/components contract with Rust examples |
| 1 | `contracts/incidents-api.md` | ✅ Complete | 500+ | POST /v2/incidents contract with FR-017 security notes |
| 1 | `contracts/request-examples/` | ✅ Complete | 2 files | JSON request payload samples |
| 1 | `contracts/response-examples/` | ✅ Complete | 2 files | JSON response payload samples |
| 1 | `quickstart.md` | ✅ Complete | 800+ | Step-by-step implementation guide with code |
| 1 | `.github/agents/copilot-instructions.md` | ✅ Updated | N/A | Agent context with project details |

**Total Documentation**: ~2,500 lines of design artifacts + code examples

### Key Design Decisions Captured

1. **Cache Architecture** (research.md § 1):
   - Nested HashMap<(String, Vec<Attr>), u32>
   - Sorted attributes for deterministic keys
   - Subset matching via iteration (O(n) acceptable for n~100)

2. **Security Model** (data-model.md § 5, contracts/incidents-api.md § FR-017):
   - Sensitive data (service names, environments, metric details) logged locally only
   - Public data (generic title/description, impact level, component IDs) sent to API
   - Clear separation documented in contracts and quickstart

3. **Error Resilience** (research.md § 5, data-model.md § "State Transitions"):
   - Startup: 3 retries × 60s delays, panic if cache load fails
   - Runtime: single cache refresh on miss, log warning if still not found
   - Incident creation: log error and continue, rely on next cycle (no immediate retry)

4. **Testing Strategy** (quickstart.md § "Testing"):
   - Unit tests: cache building, subset matching, timestamp handling
   - Integration tests: mockito for API mocking, end-to-end flow validation
   - Contract tests: OpenAPI schema validation (manual in staging)

### Constitution Re-Check (Post-Design)

**Status**: ✅ **PASS** - All principles maintained

| Principle | Re-Check Result |
|-----------|-----------------|
| I. Code Quality | ✅ Rust-idiomatic design, strong typing, proper error handling (anyhow::Result) |
| II. Testing | ✅ Comprehensive unit + integration tests planned (95% coverage target) |
| III. UX Consistency | ✅ Structured logging with FR-017 diagnostic fields, no config changes |
| IV. Performance | ✅ Component cache adds ~20KB, O(n) lookup acceptable, 10s timeout adequate |

**No new violations introduced.** Design maintains project's existing architectural patterns.

---

## Next Steps

1. **Run** `/speckit.tasks` **command** to generate task breakdown from this plan
2. **Review** generated `tasks.md` for task dependencies and estimation
3. **Begin implementation** following `quickstart.md` step-by-step guide
4. **Reference** `data-model.md` for entity structures during coding
5. **Validate** against `contracts/*.md` during API integration
6. **Execute tests** per `quickstart.md` § Testing section
7. **Update** project documentation in `doc/` after implementation

---

## References

- **Feature Spec**: [`spec.md`](spec.md) - 17 functional requirements, 3 user stories, edge cases
- **OpenAPI Schema**: `/openapi.yaml` (project root) - Status Dashboard API V2 source of truth
- **Reference Implementation**: `sd_api_v2_migration` branch - working V2 implementation for validation
- **Constitution**: `.specify/memory/constitution.md` - CloudMon Metrics Processor principles
- **Codebase**:
  - Current reporter: `src/bin/reporter.rs`
  - Health API types: `src/api/v1.rs` (ServiceHealthResponse)
  - Test fixtures: `tests/fixtures/`
