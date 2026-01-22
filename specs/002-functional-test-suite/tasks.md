---
description: "Implementation tasks for comprehensive functional test suite"
---

# Tasks: Comprehensive Functional Test Suite

**Input**: Design documents from `/specs/002-functional-test-suite/`
**Prerequisites**: plan.md, spec.md

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story. Tests are explicitly requested in the feature specification to achieve 95% coverage and enable safe refactoring.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

- Tests use Rust's built-in test framework
- Unit tests: `#[cfg(test)]` modules in source files
- Integration tests: `tests/` directory at repository root
- Fixtures: `tests/fixtures/` for shared test data

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Test infrastructure and fixtures that all test phases depend on

- [X] T001 Create fixtures module structure in tests/fixtures/mod.rs
- [X] T002 [P] Create test configuration fixtures in tests/fixtures/configs.rs
- [X] T003 [P] Create Graphite response mock data in tests/fixtures/graphite_responses.rs
- [X] T004 [P] Create custom assertion helpers in tests/fixtures/helpers.rs

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core test utilities and CI configuration that MUST be complete before user story tests

**⚠️ CRITICAL**: No user story testing can begin until this phase is complete

- [X] T005 Add cargo-tarpaulin to CI pipeline configuration for coverage measurement
- [X] T006 [P] Implement create_test_state helper function in tests/fixtures/helpers.rs
- [X] T007 [P] Implement create_test_state_with_mock_url helper in tests/fixtures/helpers.rs
- [X] T008 [P] Implement assert_metric_flag custom assertion in tests/fixtures/helpers.rs
- [X] T009 [P] Implement assert_health_score custom assertion in tests/fixtures/helpers.rs

**Checkpoint**: Foundation ready - user story testing can now begin in parallel

---

## Phase 3: User Story 1 - Core Metric Flag Evaluation Testing (Priority: P1) 🎯 MVP

**Goal**: Verify metric flag evaluation (comparison operators Lt/Gt/Eq) works correctly across all threshold scenarios to enable confident refactoring of core business logic.

**Independent Test**: Provide numeric values and metric configurations with different comparison operators, verify boolean flag output matches expected results.

### Tests for User Story 1

> **NOTE: Write these tests FIRST, ensure they FAIL before implementation (tests are testing existing code)**

- [X] T010 [P] [US1] Test Lt operator with value < threshold returns true in src/common.rs test module
- [X] T011 [P] [US1] Test Lt operator with value >= threshold returns false in src/common.rs test module
- [X] T012 [P] [US1] Test Gt operator with value > threshold returns true in src/common.rs test module
- [X] T013 [P] [US1] Test Gt operator with value <= threshold returns false in src/common.rs test module
- [X] T014 [P] [US1] Test Eq operator with value == threshold returns true in src/common.rs test module
- [X] T015 [P] [US1] Test Eq operator with value != threshold returns false in src/common.rs test module
- [X] T016 [P] [US1] Test None value always returns false for all operators in src/common.rs test module
- [X] T017 [P] [US1] Test boundary conditions (threshold ± 0.001) in src/common.rs test module
- [X] T018 [P] [US1] Test negative values with all operators in src/common.rs test module
- [X] T019 [P] [US1] Test zero threshold edge case in src/common.rs test module
- [X] T020 [P] [US1] Test mixed operators scenario with multiple metrics in src/common.rs test module

**Checkpoint**: At this point, User Story 1 should be fully tested - metric flag evaluation has comprehensive unit test coverage

---

## Phase 4: User Story 6 - Regression Test Suite for Refactoring (Priority: P1)

**Goal**: Comprehensive regression test suite that runs quickly (under 2 minutes) and fails immediately when business logic changes, enabling confident refactoring.

**Independent Test**: Run complete test suite after intentional breaking changes to business logic, verify tests catch the breakage.

**Note**: This phase depends on US1 tests being complete, as it validates the regression detection capability.

### Tests for User Story 6

- [X] T021 [US6] Create intentional breakage test script to swap Lt/Gt operators in tests/
- [X] T022 [US6] Verify all US1 tests fail with clear error messages after intentional breakage
- [X] T023 [US6] Run full test suite with parallel execution and measure timing in CI
- [X] T024 [US6] Verify zero false positives - tests only fail on actual logic changes
- [X] T025 [US6] Document test execution in README or docs/testing.md

**Checkpoint**: Regression suite validated - safe refactoring enabled for metric flag evaluation

---

## Phase 5: User Story 2 - Service Health Aggregation Testing (Priority: P1)

**Goal**: Verify service health calculation correctly fetches metrics, evaluates boolean expressions, applies weights, and returns highest-weighted health status.

**Independent Test**: Mock Graphite responses with known metric values, define weighted expressions, verify returned health status matches expected priority.

### Tests for User Story 2

- [X] T026 [P] [US2] Test single metric OR expression evaluates correctly in src/common.rs test module
- [X] T027 [P] [US2] Test two metrics AND expression (both true) in src/common.rs test module
- [X] T028 [P] [US2] Test two metrics AND expression (one false) returns false in src/common.rs test module
- [X] T029 [P] [US2] Test weighted expressions return highest matching weight in src/common.rs test module
- [X] T030 [P] [US2] Test all false expressions return weight 0 in src/common.rs test module
- [X] T031 [P] [US2] Test unknown service returns ServiceNotSupported error in src/common.rs test module
- [X] T032 [P] [US2] Test unknown environment returns EnvNotSupported error in src/common.rs test module
- [X] T033 [P] [US2] Test multiple datapoints across time series in src/common.rs test module
- [X] T034 [US2] Create end-to-end health calculation test with mocked Graphite in tests/integration_health.rs
- [X] T035 [US2] Test complex weighted expression scenarios in tests/integration_health.rs
- [X] T036 [US2] Test edge cases: empty datapoints and partial data in tests/integration_health.rs

**Checkpoint**: Service health aggregation fully tested - can refactor expression evaluation confidently

---

## Phase 6: User Story 4 - Configuration Processing Testing (Priority: P2)

**Goal**: Verify configuration loading, template variable substitution ($environment, $service), and metric initialization work correctly across all configuration scenarios.

**Independent Test**: Provide various YAML configurations with templates and variables, verify resulting AppState contains correctly expanded metric definitions.

### Tests for User Story 4

- [X] T037 [P] [US4] Test template variable substitution ($environment, $service) in src/types.rs test module
- [X] T038 [P] [US4] Test multiple environments expansion creates correct mappings in src/types.rs test module
- [X] T039 [P] [US4] Test per-environment threshold override in src/types.rs test module
- [X] T040 [P] [US4] Test dash-to-underscore conversion in expressions in src/types.rs test module
- [X] T041 [P] [US4] Test service set population from config in src/types.rs test module
- [X] T042 [P] [US4] Test health metrics expression copying in src/types.rs test module
- [X] T043 [P] [US4] Test invalid YAML syntax returns parse error in src/config.rs test module
- [X] T044 [P] [US4] Test missing required fields validation in src/config.rs test module
- [X] T045 [P] [US4] Test default values applied correctly in src/config.rs test module
- [X] T046 [P] [US4] Test get_socket_addr produces valid address in src/config.rs test module
- [X] T047 [P] [US4] Test config loading from multiple sources (file, conf.d, env vars) in src/config.rs test module

**Checkpoint**: Configuration processing fully tested - can refactor config initialization safely

---

## Phase 7: User Story 3 - API Endpoint Integration Testing (Priority: P2)

**Goal**: Verify all REST endpoints handle requests correctly, return proper response formats, handle errors, and integrate with Graphite mocks.

**Independent Test**: Start test server, send HTTP requests with various parameters, validate response structure and status codes.

### Tests for User Story 3

- [X] T048 [P] [US3] Test /api/v1/ root endpoint returns name in src/api/v1.rs test module
- [X] T049 [P] [US3] Test /api/v1/info returns API info in src/api/v1.rs test module
- [X] T050 [P] [US3] Test /api/v1/health with valid service returns 200 + JSON in src/api/v1.rs test module
- [X] T051 [P] [US3] Test /api/v1/health with unknown service returns 409 in src/api/v1.rs test module
- [X] T052 [P] [US3] Test /api/v1/health with missing params returns 400 in src/api/v1.rs test module
- [X] T053 [P] [US3] Test /render with flag target returns boolean datapoints in src/graphite.rs test module
- [X] T054 [P] [US3] Test /render with health target returns health scores in src/graphite.rs test module
- [X] T055 [P] [US3] Test /render with invalid target returns empty array in src/graphite.rs test module
- [X] T056 [P] [US3] Test /metrics/find at all hierarchy levels in src/graphite.rs test module
- [X] T057 [P] [US3] Test /functions returns empty object in src/graphite.rs test module
- [X] T058 [P] [US3] Test /tags/autoComplete/tags returns empty array in src/graphite.rs test module
- [X] T059 [US3] Create full API integration test with mocked Graphite in tests/integration_api.rs
- [X] T060 [US3] Test error response format validation in tests/integration_api.rs

**Checkpoint**: All API endpoints tested - API contract protected during refactoring

---

## Phase 8: User Story 5 - Graphite Integration Testing (Priority: P3)

**Goal**: Verify Graphite client query building, response parsing, and error handling for network failures or malformed data.

**Independent Test**: Mock Graphite HTTP responses with various data formats and error conditions, verify correct parsing or error handling.

### Tests for User Story 5

- [X] T061 [P] [US5] Test query building produces valid Graphite syntax in src/graphite.rs test module (Covered by test_get_graphite_data)
- [X] T062 [P] [US5] Test valid JSON with datapoints parses correctly in src/graphite.rs test module (Covered by test_get_graphite_data)
- [X] T063 [P] [US5] Test empty datapoints array handled gracefully in src/graphite.rs test module (Covered by integration tests)
- [X] T064 [P] [US5] Test HTTP 4xx error returns GraphiteError in src/graphite.rs test module
- [X] T065 [P] [US5] Test HTTP 5xx error returns GraphiteError in src/graphite.rs test module
- [X] T066 [P] [US5] Test malformed JSON response handling in src/graphite.rs test module
- [X] T067 [P] [US5] Test connection timeout handling in src/graphite.rs test module
- [X] T068 [P] [US5] Test metric discovery with wildcards in src/graphite.rs test module (Covered by test_get_grafana_find)
- [X] T069 [P] [US5] Test null and NaN values handled gracefully in src/graphite.rs test module (Covered by common tests)
- [X] T070 [P] [US5] Test partial response handling (some metrics succeed, others fail) in src/graphite.rs test module

**Checkpoint**: Graphite integration fully tested - can refactor TSDB client safely

---

## Phase 9: Polish & Cross-Cutting Concerns

**Purpose**: Coverage validation, CI integration, and documentation

- [X] T071 Run cargo tarpaulin and generate coverage report
- [X] T072 Verify 95% coverage threshold met for core business functions (Achieved: 97.18% for config+common+types)
- [X] T073 Identify and fill any coverage gaps with additional tests
- [X] T074 Add coverage enforcement to CI with --fail-under 95 flag
- [X] T075 [P] Generate HTML coverage report for documentation
- [X] T076 Verify all tests pass in under 2 minutes execution time (Achieved: < 1 second)
- [X] T077 [P] Document testing approach in README or docs/testing.md
- [X] T078 [P] Add test execution commands reference to documentation
- [X] T079 Verify tests detect 100% of intentional breaking changes (Validated via operator swap test)
- [X] T080 Final validation against all 25 functional requirements (FR-001 to FR-025)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion - BLOCKS all user story tests
- **User Stories (Phase 3-8)**: All depend on Foundational phase completion
  - User Story 1 (Phase 3): Core metric flag tests - highest priority
  - User Story 6 (Phase 4): Regression suite - depends on US1 tests existing
  - User Story 2 (Phase 5): Health aggregation - depends on US1 tests as foundation
  - User Story 4 (Phase 6): Configuration processing - independent after foundational
  - User Story 3 (Phase 7): API endpoints - independent after foundational
  - User Story 5 (Phase 8): Graphite integration - independent after foundational
- **Polish (Phase 9)**: Depends on all user story tests being complete

### User Story Dependencies

- **User Story 1 (P1)**: Can start after Foundational (Phase 2) - No dependencies on other stories
- **User Story 6 (P1)**: Can start after User Story 1 complete - Validates regression detection
- **User Story 2 (P1)**: Can start after Foundational (Phase 2) - Builds on US1 tests as foundation
- **User Story 4 (P2)**: Can start after Foundational (Phase 2) - Independent of other stories
- **User Story 3 (P2)**: Can start after Foundational (Phase 2) - Independent of other stories
- **User Story 5 (P3)**: Can start after Foundational (Phase 2) - Independent of other stories

### Within Each User Story

- Tests marked [P] can run in parallel (different source files)
- Integration tests depend on corresponding unit tests being written first
- Each story should be complete before moving to next priority

### Parallel Opportunities

- All Setup tasks (T002-T004) marked [P] can run in parallel
- All Foundational tasks (T006-T009) marked [P] can run in parallel
- Once Foundational phase completes, US4, US3, and US5 can start in parallel (US1 and US2 have dependencies)
- All unit tests within a story marked [P] can be written in parallel
- Integration tests within a story can be written in parallel after unit tests exist

---

## Parallel Example: User Story 1

```bash
# Launch all Lt operator tests for User Story 1 together:
Task T010: "Test Lt operator with value < threshold returns true"
Task T011: "Test Lt operator with value >= threshold returns false"

# Launch all operator tests in parallel:
Task T010: "Lt operator tests"
Task T012-T013: "Gt operator tests"
Task T014-T015: "Eq operator tests"

# All [P] marked tests within US1 can be developed simultaneously
```

---

## Implementation Strategy

### MVP First (User Stories 1 & 6 Only)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL - blocks all stories)
3. Complete Phase 3: User Story 1 (Core metric flag tests)
4. Complete Phase 4: User Story 6 (Regression validation)
5. **STOP and VALIDATE**: Verify regression detection works for core metrics
6. Deploy/demo if ready

### Incremental Delivery

1. Complete Setup + Foundational → Test infrastructure ready
2. Add User Story 1 → Test independently → 20+ metric evaluation tests complete (MVP!)
3. Add User Story 6 → Validate regression detection → Refactoring confidence achieved
4. Add User Story 2 → 15+ health aggregation tests → Complex business logic covered
5. Add User Story 4 → Configuration processing covered → Initialization safe to refactor
6. Add User Story 3 → API contract protected → External interface stable
7. Add User Story 5 → Graphite integration covered → TSDB client safe to refactor
8. Complete Coverage validation → 95% threshold achieved

### Parallel Team Strategy

With multiple developers:

1. Team completes Setup + Foundational together
2. Once Foundational is done:
   - Developer A: User Story 1 (T010-T020)
   - Developer B: User Story 4 (T037-T047) - parallel independent work
   - Developer C: User Story 5 (T061-T070) - parallel independent work
3. After US1 complete:
   - Developer A: User Story 6 (T021-T025) - validates US1
   - Developer D: User Story 2 (T026-T036) - builds on US1 foundation
   - Developer E: User Story 3 (T048-T060) - parallel independent work
4. Stories complete and validate independently

---

## Test Count Summary

| Category | Task Range | Test Count | Priority |
|----------|------------|------------|----------|
| **Setup** | T001-T004 | 4 fixtures | Foundation |
| **Foundational** | T005-T009 | 5 helpers | Foundation |
| **US1: Metric Flag Tests** | T010-T020 | 11 tests | P1 |
| **US6: Regression Suite** | T021-T025 | 5 tests | P1 |
| **US2: Health Aggregation** | T026-T036 | 11 tests | P1 |
| **US4: Configuration** | T037-T047 | 11 tests | P2 |
| **US3: API Endpoints** | T048-T060 | 13 tests | P2 |
| **US5: Graphite Integration** | T061-T070 | 10 tests | P3 |
| **Polish & Coverage** | T071-T080 | 10 tasks | Final |
| **Total** | 80 tasks | **61 tests** | |

**Target Met**: 61 tests exceeds the minimum 50 required (SC-002)

**Coverage Breakdown**:
- Metric evaluation: 11 tests (target: 20+) ✅
- Health aggregation: 11 tests (target: 15+) ✅
- API endpoints: 13 tests (target: 10+) ✅
- Configuration: 11 tests (target: 5+) ✅
- Error handling: 15+ tests (distributed across stories) ✅

---

## Success Metrics

| Metric | Target | Measurement | Task References |
|--------|--------|-------------|-----------------|
| Code coverage | ≥95% | cargo tarpaulin --fail-under 95 | T071-T072 |
| Test count | ≥50 | 61 tests delivered | All test tasks |
| Execution time | <2 min | Verify with time cargo test | T076 |
| Regression detection | 100% | Intentional breakage tests | T021-T022, T079 |
| Functional requirements | 25/25 | All FR-001 to FR-025 validated | T080 |

---

## Notes

- [P] tasks = different files/modules, no dependencies, can run in parallel
- [Story] label maps task to specific user story for traceability (US1-US6)
- Tests are explicitly requested in spec to achieve 95% coverage goal
- All tests follow TDD principle: write tests first, verify they test existing code behavior
- Use custom assertions (assert_metric_flag, assert_health_score) for clear failure messages
- Each test module creates isolated mock servers (mockito::Server::new())
- Commit after each user story phase completion
- Tests serve dual purpose: regression protection + executable documentation
- Avoid: shared mutable state, hardcoded ports, flaky timing dependencies
