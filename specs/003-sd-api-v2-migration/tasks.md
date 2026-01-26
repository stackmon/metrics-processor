# Tasks: Status Dashboard API V2 Migration

**Input**: Design documents from `/specs/003-sd-api-v2-migration/`
**Prerequisites**: plan.md (tech stack), spec.md (3 user stories: P1, P2, P3), research.md (cache design), data-model.md (entities), contracts/ (API endpoints)

**Tests**: Not requested in feature specification - focusing on implementation tasks

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `- [ ] [ID] [P?] [Story] Description`

- **Checkbox**: ALWAYS start with `- [ ]`
- **[ID]**: Task ID (T001, T002, etc.)
- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1, US2, US3)
- Include exact file paths in descriptions

## Path Conventions

**Single Rust project** at repository root:
- `src/bin/reporter.rs` - main reporter implementation
- `tests/reporter_v2_integration.rs` - integration tests
- `Cargo.toml` - dependency management

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Project initialisation and dependency updates

- [x] T001 Add anyhow ~1.0 dependency to Cargo.toml for Result error handling
- [x] T002 [P] Update reqwest client timeout from 2s to 10s in src/bin/reporter.rs per FR-014

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core data structures and utilities that ALL user stories depend on

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [ ] T003 [P] Add StatusDashboardComponent struct in src/bin/reporter.rs for V2 API response
- [ ] T004 [P] Update ComponentAttribute with PartialOrd, Ord, Hash derives in src/bin/reporter.rs
- [ ] T005 [P] Add IncidentData struct in src/bin/reporter.rs for V2 incident payload
- [ ] T006 Create ComponentCache type alias HashMap<String, HashMap<String, u64>> in src/bin/reporter.rs

**Checkpoint**: Foundation ready - all user stories can now proceed

---

## Phase 3: User Story 1 - Reporter Creates Incidents via V2 API (Priority: P1) 🎯 MVP

**Goal**: Enable reporter to create incidents using the new V2 API endpoint while maintaining monitoring capabilities

**Independent Test**: Trigger a service health issue (impact > 0) and verify incident appears in Status Dashboard with correct component ID, impact, and timestamp

**FR Coverage**: FR-001, FR-002, FR-009, FR-010, FR-011, FR-013, FR-016, FR-017

### Implementation for User Story 1

- [ ] T007 [P] [US1] Implement fetch_components() async function in src/bin/reporter.rs to call GET /v2/components
- [ ] T008 [P] [US1] Implement build_component_id_cache() function in src/bin/reporter.rs to construct nested HashMap
- [ ] T009 [US1] Implement find_component_id() function in src/bin/reporter.rs with subset attribute matching per FR-012
- [ ] T010 [US1] Implement build_incident_data() function in src/bin/reporter.rs with static title/description per FR-002
- [ ] T011 [US1] Add timestamp handling with RFC3339 format and -1 second adjustment in build_incident_data() per FR-011
- [ ] T012 [US1] Implement create_incident() async function in src/bin/reporter.rs to POST /v2/incidents
- [ ] T013 [US1] Update metric_watcher() to replace V1 endpoint (/v1/component_status) with V2 incident creation
- [ ] T014 [US1] Add structured logging with diagnostic fields (timestamp, service, environment, component details, impact, triggered_metrics) per FR-017
- [ ] T015 [US1] Add error logging for incident creation failures with status and response body per FR-015

**Checkpoint**: User Story 1 complete - reporter can create incidents via V2 API

---

## Phase 4: User Story 2 - Component Cache Management (Priority: P2)

**Goal**: Maintain cache mapping component names to IDs with automatic refresh when components not found

**Independent Test**: Start reporter, add new component to Status Dashboard, trigger issue for that component, verify reporter refreshes cache and creates incident

**FR Coverage**: FR-003, FR-004, FR-005, FR-012

### Implementation for User Story 2

- [ ] T016 [US2] Add component cache initialization in metric_watcher() to fetch and build cache at startup
- [ ] T017 [US2] Implement cache miss detection in metric_watcher() when component not found during lookup
- [ ] T018 [US2] Implement single cache refresh attempt (call fetch_components + rebuild cache) on cache miss per FR-005
- [ ] T019 [US2] Add warning logging when component still not found after cache refresh per FR-015
- [ ] T020 [US2] Add continue to next service logic when component cannot be resolved (no retry on incident creation)

**Checkpoint**: User Story 2 complete - cache management with automatic refresh working

---

## Phase 5: User Story 3 - Authorization Remains Unchanged (Priority: P3)

**Goal**: Verify existing HMAC-JWT authorization works with V2 endpoints without any changes

**Independent Test**: Verify reporter uses existing secret to generate JWT token and successfully authenticates with V2 endpoints

**FR Coverage**: FR-008

### Implementation for User Story 3

- [ ] T021 [US3] Verify existing HMAC-JWT token generation in metric_watcher() is reused for V2 endpoints
- [ ] T022 [US3] Verify Authorization header format remains unchanged (Bearer {jwt-token}) for V2 API calls
- [ ] T023 [US3] Test that reporter operates without auth headers when no secret configured (optional auth)

**Checkpoint**: User Story 3 complete - authorization verified unchanged for V2

---

## Phase 6: Startup Reliability & Error Handling

**Goal**: Add robust error handling for startup cache loading with retry logic

**FR Coverage**: FR-006, FR-007

- [ ] T024 Add initial component cache load with 3 retry attempts in metric_watcher() per FR-006
- [ ] T025 Add 60-second delay between cache load retry attempts per FR-006
- [ ] T026 Add error return from metric_watcher() if cache load fails after 3 attempts per FR-007
- [ ] T027 Add warning logging for each failed cache load attempt with attempt number

**Checkpoint**: Startup reliability complete - reporter handles API unavailability

---

## Phase 7: Integration Testing

**Purpose**: Validate end-to-end V2 migration with mocked API endpoints

- [ ] T028 [P] Create tests/reporter_v2_integration.rs test file with mockito setup
- [ ] T029 [P] Add test_fetch_components_success() to verify component fetching and parsing
- [ ] T030 [P] Add test_build_component_id_cache() to verify cache structure with nested HashMap
- [ ] T031 [P] Add test_find_component_id_subset_matching() to verify FR-012 subset attribute matching
- [ ] T032 [P] Add test_build_incident_data_structure() to verify static title/description per FR-002
- [ ] T033 [P] Add test_timestamp_rfc3339_minus_one_second() to verify FR-011 timestamp handling
- [ ] T034 [P] Add test_create_incident_success() to verify POST /v2/incidents with mockito
- [ ] T035 [P] Add test_cache_refresh_on_miss() to verify FR-005 single refresh attempt
- [ ] T036 [P] Add test_startup_retry_logic() to verify FR-006 3 retry attempts with delays
- [ ] T037 [P] Add test_error_logging_with_diagnostic_fields() to verify FR-017 structured logging

**Checkpoint**: Integration tests complete - all V2 functionality validated

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Code quality, documentation, and final validation

- [ ] T038 [P] Run cargo fmt to format all code changes
- [ ] T039 [P] Run cargo clippy to check for lints and warnings
- [ ] T040 Run cargo test to execute all tests including new integration tests
- [ ] T041 Run cargo build to verify compilation without errors
- [ ] T042 [P] Update comments and doc strings in src/bin/reporter.rs for new functions
- [ ] T043 Verify quickstart.md steps match actual implementation
- [ ] T044 [P] Add inline comments explaining cache structure and subset matching logic
- [ ] T045 Review all error messages for clarity and actionability per Constitution III

**Checkpoint**: Feature ready for code review and deployment

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on T001, T002 - BLOCKS all user stories
- **User Story 1 (Phase 3)**: Depends on Foundational (T003-T006) complete
- **User Story 2 (Phase 4)**: Depends on User Story 1 (T007-T015) complete
- **User Story 3 (Phase 5)**: Depends on User Story 1 (T007-T015) complete (verification only)
- **Startup Reliability (Phase 6)**: Depends on User Story 2 (T016-T020) complete
- **Integration Testing (Phase 7)**: Depends on all implementation phases (T003-T027) complete
- **Polish (Phase 8)**: Depends on all previous phases complete

### User Story Dependencies

- **User Story 1 (P1)**: Foundation → Core V2 incident creation (REQUIRED for MVP)
- **User Story 2 (P2)**: User Story 1 → Add cache refresh logic (enhances US1)
- **User Story 3 (P3)**: User Story 1 → Verification of auth (depends on US1 endpoints)

### Within Each User Story

**User Story 1**:
- T007, T008 can run in parallel (different functions)
- T009 depends on T008 (uses cache structure)
- T010, T011 can run after T009 (needs component resolution)
- T012 depends on T010 (uses IncidentData struct)
- T013 depends on T007-T012 (integrates all functions)
- T014, T015 can run in parallel with T013 (logging is separate)

**User Story 2**:
- T016-T020 are sequential (modify metric_watcher flow)

**User Story 3**:
- T021-T023 are verification tasks (can run in parallel)

**Integration Testing**:
- All tests (T028-T037) marked [P] can run in parallel

### Parallel Opportunities

- **Phase 1 Setup**: T001, T002 can run in parallel (different concerns)
- **Phase 2 Foundational**: T003, T004, T005 can run in parallel (different structs)
- **Phase 3 User Story 1**: T007, T008 can run in parallel initially
- **Phase 7 Integration Testing**: T029-T037 all marked [P] can run simultaneously
- **Phase 8 Polish**: T038, T039, T042, T044, T045 marked [P] can run simultaneously

---

## Parallel Example: Foundational Phase

```bash
# Launch all struct definitions together:
Task T003: "Add StatusDashboardComponent struct in src/bin/reporter.rs"
Task T004: "Update ComponentAttribute derives in src/bin/reporter.rs"
Task T005: "Add IncidentData struct in src/bin/reporter.rs"
```

## Parallel Example: Integration Testing

```bash
# Launch all integration tests together:
Task T029: "Add test_fetch_components_success()"
Task T030: "Add test_build_component_id_cache()"
Task T031: "Add test_find_component_id_subset_matching()"
Task T032: "Add test_build_incident_data_structure()"
Task T033: "Add test_timestamp_rfc3339_minus_one_second()"
# ... and so on for all test tasks
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001-T002)
2. Complete Phase 2: Foundational (T003-T006) - CRITICAL
3. Complete Phase 3: User Story 1 (T007-T015)
4. **STOP and VALIDATE**: Test incident creation via V2 API manually
5. MVP READY: Reporter can create incidents using V2 endpoint

**Estimated Tasks for MVP**: 17 tasks (T001-T015 + T001-T002 foundational)

### Incremental Delivery

1. MVP (US1) → Deploy/Demo → Reporter creates V2 incidents ✅
2. Add US2 (T016-T020) → Deploy/Demo → Cache refresh on miss ✅
3. Add US3 (T021-T023) → Deploy/Demo → Auth verified ✅
4. Add Startup Reliability (T024-T027) → Deploy/Demo → Robust startup ✅
5. Add Testing (T028-T037) → Full test coverage ✅
6. Polish (T038-T045) → Production ready ✅

### Critical Path

**Blocking sequence** (cannot parallelize):
1. T001-T002 (Setup) → T003-T006 (Foundation) → T009 (component lookup) → T012 (incident creation) → T013 (integration) → T024-T027 (startup reliability)

**Total Critical Path**: ~13 tasks that MUST be sequential

**Parallelizable**: ~32 tasks that can run in parallel (all [P] marked tasks)

---

## Task Summary

| Phase | Task Count | Parallelizable | User Story |
|-------|------------|----------------|------------|
| Phase 1: Setup | 2 | 1 | N/A |
| Phase 2: Foundational | 4 | 3 | N/A |
| Phase 3: User Story 1 | 9 | 2 | P1 (MVP) |
| Phase 4: User Story 2 | 5 | 0 | P2 |
| Phase 5: User Story 3 | 3 | 3 | P3 |
| Phase 6: Startup Reliability | 4 | 0 | N/A |
| Phase 7: Integration Testing | 10 | 10 | N/A |
| Phase 8: Polish | 8 | 4 | N/A |
| **TOTAL** | **45** | **23** | **3 stories** |

### Task Distribution by User Story

- **User Story 1 (P1)**: 9 tasks - Core V2 incident creation 🎯 MVP
- **User Story 2 (P2)**: 5 tasks - Cache management with refresh
- **User Story 3 (P3)**: 3 tasks - Authorization verification
- **Infrastructure**: 18 tasks - Setup, foundation, testing, polish
- **Parallel Opportunities**: 23 tasks (51%) can run simultaneously

### Independent Test Criteria

**User Story 1**: Manually trigger service health issue with impact > 0 → verify incident created in Status Dashboard → check component ID, impact level, timestamp, system=true flag

**User Story 2**: Start reporter → add new component in Status Dashboard → trigger issue for new component → verify logs show cache refresh → verify incident created successfully

**User Story 3**: Review code that no auth changes made → verify JWT token generation unchanged → verify Authorization header format unchanged → test with/without secret configuration

---

## Suggested MVP Scope

**MVP = User Story 1 only** (17 tasks: T001-T015)

Delivers core value:
✅ Reporter creates incidents via V2 API
✅ Component ID resolution from cache
✅ Static secure incident payloads
✅ Structured diagnostic logging
✅ Error handling for incident creation

Not in MVP (can add later):
⏸️ Automatic cache refresh on miss (US2)
⏸️ Auth verification tasks (US3)  
⏸️ Startup retry logic (Phase 6)
⏸️ Integration tests (Phase 7)

**Rationale**: US1 provides immediate business value - reporter works with V2 API. US2/US3 are enhancements that can be added incrementally.

---

## Notes

- All tasks follow checklist format: `- [ ] [ID] [P?] [Story?] Description with file path`
- [P] tasks target different files or independent functions
- [Story] labels (US1, US2, US3) map to spec.md priorities (P1, P2, P3)
- Each user story independently testable per acceptance scenarios in spec.md
- Constitution compliance: Rust idioms, anyhow::Result, structured logging, 95% test coverage target
- Reference implementations: quickstart.md (step-by-step guide), contracts/ (API specs)
