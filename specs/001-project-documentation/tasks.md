# Tasks: Comprehensive Project Documentation

**Input**: Design documents from `/specs/001-project-documentation/`  
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Validation tests are included per research.md findings (cargo test, mdbook-linkcheck, example validation)

**Organization**: Tasks are grouped by user story to enable independent implementation and testing of each story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (e.g., US1, US2, US3)
- Include exact file paths in descriptions

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Install tooling and configure infrastructure for documentation generation

- [X] T001 Install mdbook and plugins: `cargo install mdbook mdbook-mermaid mdbook-linkcheck`
- [X] T002 Update doc/book.toml with preprocessor configuration for mermaid and linkcheck
- [X] T003 [P] Add schemars dependency to Cargo.toml for JSON schema generation
- [X] T004 [P] Update doc/SUMMARY.md with new documentation sections structure

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core documentation infrastructure that MUST be complete before user story documentation can be created

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [X] T006 Create build.rs script to auto-generate doc/schemas/config-schema.json from src/config.rs
- [X] T007 [P] Copy specs/001-project-documentation/contracts/patterns.json to doc/schemas/patterns.json
- [X] T008 [P] Create doc/schemas/README.md explaining schema usage for AI tools and IDEs
- [X] T009 Create validation test framework in tests/documentation_validation.rs
- [X] T010 [P] Add test function to validate all YAML examples parse correctly
- [X] T011 [P] Add test function to validate schema matches Config struct definition
- [X] T012 Enhance doc/index.md with comprehensive project overview per FR-001

**Checkpoint**: Foundation ready - user story documentation can now be created in parallel

---

## Phase 3: User Story 1 - New Developer Onboarding (Priority: P1) 🎯 MVP

**Goal**: New developers understand project purpose, set up environment, and locate components within 30 minutes

**Independent Test**: A developer unfamiliar with the project follows only the onboarding documentation to set up environment, understand project purpose, and locate convertor/reporter components without external help

### Validation for User Story 1

- [X] T013 [US1] Add test to validate quickstart.md examples compile and run in tests/documentation_validation.rs

### Implementation for User Story 1

- [X] T014 [P] [US1] Migrate specs/001-project-documentation/quickstart.md to doc/getting-started/quickstart.md
- [X] T015 [P] [US1] Create doc/getting-started/project-structure.md documenting src/ layout and module organization
- [X] T016 [US1] Create doc/getting-started/development.md with testing, debugging, and workflow instructions
- [X] T017 [US1] Add getting-started section entries to doc/SUMMARY.md

**Checkpoint**: Developer onboarding documentation complete - new developers can successfully set up environment independently

---

## Phase 4: User Story 2 - AI-Assisted Development (Priority: P1)

**Goal**: AI agents and IDE assistants understand project structure, conventions, and APIs to provide accurate code suggestions

**Independent Test**: Provide only documentation to an AI agent and ask it to generate code for adding a new TSDB backend. Success means generated code follows project conventions without guidance beyond documentation.

### Implementation for User Story 2

- [X] T018 [P] [US2] Run build.rs to generate doc/schemas/config-schema.json from Config struct
- [X] T019 [P] [US2] Create doc/schemas/types.json with type definitions for all core data structures
- [X] T020 [US2] Verify patterns.json includes all naming conventions and code patterns
- [X] T021 [US2] Update .github/agents/copilot-instructions.md with links to new schemas
- [X] T022 [US2] Add schema validation test ensuring schemas match Rust source in tests/documentation_validation.rs

**Checkpoint**: AI/IDE integration complete - tools can access machine-readable schemas for accurate code generation

---

## Phase 5: User Story 6 - Architecture Understanding (Priority: P2)

**Goal**: Developers understand system design, data flow, and patterns to make safe architectural decisions

**Independent Test**: A senior developer designs a performance optimization using only architecture documentation, respecting existing patterns and correctly identifying bottlenecks

### Implementation for User Story 6

- [X] T023 [P] [US6] Create doc/architecture/overview.md describing convertor, reporter, TSDB, dashboard relationships
- [X] T024 [P] [US6] Create doc/architecture/diagrams.md with Mermaid system architecture diagram
- [X] T025 [P] [US6] Add Mermaid module dependency graph to doc/architecture/diagrams.md
- [X] T026 [US6] Create doc/architecture/data-flow.md documenting TSDB → flag metrics → health metrics flow with sequence diagram
- [X] T027 [US6] Enhance doc/convertor.md with detailed flag metric evaluation process
- [X] T028 [US6] Enhance doc/reporter.md with polling logic and dashboard integration details
- [X] T029 [US6] Add architecture section entries to doc/SUMMARY.md

**Checkpoint**: Architecture documentation complete - developers can understand system design and data flow

---

## Phase 6: User Story 3 - API Integration (Priority: P2)

**Goal**: External teams integrate with metrics-processor API with clear endpoint documentation, examples, and error handling

**Independent Test**: Developer unfamiliar with project builds an API client that queries health metrics using only API documentation, without consulting source code

### Validation for User Story 3

- [X] T030 [US3] Add test to validate API documentation matches openapi-schema.yaml in tests/documentation_validation.rs

### Implementation for User Story 3

- [X] T031 [P] [US3] Create doc/api/endpoints.md documenting /v1/health and /v1/maintenances from openapi-schema.yaml
- [X] T032 [P] [US3] Create doc/api/authentication.md documenting JWT token mechanism for status dashboard
- [X] T033 [US3] Create doc/api/examples.md with request/response samples for common scenarios
- [X] T034 [US3] Add API section entries to doc/SUMMARY.md

**Checkpoint**: API documentation complete - external developers can integrate without source code access

---

## Phase 7: User Story 4 - Configuration Management (Priority: P2)

**Goal**: Operations teams and developers configure metrics-processor with comprehensive reference, examples, and troubleshooting

**Independent Test**: Operations engineer configures monitoring for new service with custom flag metrics using only configuration documentation, producing valid configuration without trial-and-error

### Validation for User Story 4

- [X] T035 [US4] Add test to validate all configuration examples parse and conform to schema in tests/documentation_validation.rs

### Implementation for User Story 4

- [X] T036 [P] [US4] Create doc/configuration/overview.md with configuration structure introduction
- [X] T037 [P] [US4] Create doc/configuration/schema.md referencing auto-generated JSON schema with field descriptions
- [X] T038 [P] [US4] Create doc/configuration/datasource.md documenting TSDB connection configuration
- [X] T039 [P] [US4] Create doc/configuration/metric-templates.md documenting query templates and variable substitution
- [X] T040 [P] [US4] Create doc/configuration/flag-metrics.md documenting flag metric configuration and comparison operators
- [X] T041 [P] [US4] Create doc/configuration/health-metrics.md documenting health expressions and boolean operators
- [X] T042 [P] [US4] Create doc/configuration/environments.md documenting environment configuration
- [X] T043 [US4] Create doc/configuration/examples.md with working configuration samples for common scenarios
- [X] T044 [US4] Add configuration section entries to doc/SUMMARY.md

**Checkpoint**: Configuration documentation complete - operations teams can configure without errors

---

## Phase 8: User Story 5 - TSDB Backend Extension (Priority: P3)

**Goal**: Developers add support for new TSDB backends with clear integration interfaces and testing guidance

**Independent Test**: Developer implements Prometheus backend using only integration documentation, implementing correct traits and query translation without source code archaeology

### Implementation for User Story 5

- [X] T045 [P] [US5] Create doc/integration/interface.md defining TSDB trait requirements and responsibilities
- [X] T046 [P] [US5] Create doc/integration/graphite.md documenting existing Graphite implementation as reference
- [X] T047 [US5] Create doc/integration/adding-backends.md with step-by-step guide for new backends
- [X] T048 [US5] Add integration section entries to doc/SUMMARY.md

**Checkpoint**: Integration documentation complete - developers can extend TSDB support independently

---

## Phase 9: Module Documentation (Supporting Multiple Stories)

**Goal**: Document Rust module structure and responsibilities to support code navigation and understanding

**Independent Test**: Developer identifies which module to edit for specific changes using only module documentation

### Implementation for Module Documentation

- [X] T049 [P] Create doc/modules/overview.md with module responsibility matrix
- [X] T050 [P] Create doc/modules/api.md documenting HTTP API module and axum handlers
- [X] T051 [P] Create doc/modules/config.md documenting configuration parsing and validation logic
- [X] T052 [P] Create doc/modules/types.md documenting core data structures (Config, FlagMetric, HealthMetric)
- [X] T053 [P] Create doc/modules/graphite.md documenting Graphite TSDB integration implementation
- [X] T054 [P] Create doc/modules/common.md documenting shared utilities and helper functions
- [X] T055 Create doc/modules/ section entries to doc/SUMMARY.md

**Checkpoint**: Module documentation complete - developers understand code organization

---

## Phase 10: Operational Guides (Supporting Multiple Stories)

**Goal**: Provide troubleshooting and deployment guidance for operations teams and developers

**Independent Test**: Operations engineer resolves configuration error using troubleshooting guide without external support

### Implementation for Operational Guides

- [X] T056 [P] Create doc/guides/troubleshooting.md with common issues, error messages, and solutions
- [X] T057 [P] Create doc/guides/deployment.md with deployment patterns and configuration tips
- [X] T058 Create doc/guides/ section entries to doc/SUMMARY.md

**Checkpoint**: Operational guides complete - teams can troubleshoot and deploy independently

---

## Phase 11: Validation & Testing

**Purpose**: Ensure all documentation is accurate, links work, and examples are valid

- [X] T059 Run cargo test --doc to validate Rust code examples compile
- [X] T060 Run cargo test --test documentation_validation to validate YAML examples and schemas
- [X] T061 Run mdbook test doc/ to validate markdown links with mdbook-linkcheck
- [X] T062 Run mdbook build doc/ to ensure documentation builds without errors
- [X] T063 Manually verify all diagrams render correctly in browser
- [X] T064 Update CI pipeline configuration to include documentation validation tests

**Checkpoint**: All documentation validated - examples work, links resolve, schemas match code

---

## Phase 12: Navigation & Polish

**Purpose**: Final improvements for discoverability and user experience

- [X] T065 Review and refine doc/SUMMARY.md table of contents for logical flow
- [X] T066 Add cross-references between related documentation pages
- [X] T067 Ensure consistent terminology across all documentation sections
- [X] T068 Add search keywords to page titles for better discoverability
- [X] T069 Proofread all content for clarity, grammar, and accuracy
- [X] T070 Generate final documentation with `mdbook build doc/` and review in browser
- [X] T071 Run quickstart.md validation with fresh developer environment

**Checkpoint**: Documentation complete, polished, and ready for use

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies - can start immediately
- **Foundational (Phase 2)**: Depends on Setup completion (T001-T005) - BLOCKS all user stories
- **User Stories (Phase 3-8)**: All depend on Foundational phase completion (T006-T012)
  - US1 (Phase 3): Can start after Foundational
  - US2 (Phase 4): Can start after Foundational
  - US6 (Phase 5): Can start after Foundational
  - US3 (Phase 6): Can start after Foundational
  - US4 (Phase 7): Can start after Foundational
  - US5 (Phase 8): Can start after Foundational
- **Module Docs (Phase 9)**: Can start after Foundational, supports all user stories
- **Operational Guides (Phase 10)**: Can start after Foundational, supports US4
- **Validation (Phase 11)**: Depends on all documentation content being written
- **Polish (Phase 12)**: Depends on Validation completion

### User Story Dependencies

- **User Story 1 (P1)**: No dependencies on other stories - can proceed independently
- **User Story 2 (P1)**: No dependencies on other stories - can proceed independently
- **User Story 6 (P2)**: No dependencies on other stories - can proceed independently
- **User Story 3 (P2)**: No dependencies on other stories - can proceed independently
- **User Story 4 (P2)**: No dependencies on other stories - can proceed independently
- **User Story 5 (P3)**: No dependencies on other stories - can proceed independently

All user stories are independently implementable after Foundational phase completion.

### Within Each Phase

- Setup: All [P] tasks can run in parallel (T003, T004, T005)
- Foundational: [P] tasks can run in parallel (T007-T008, T010-T011)
- User Stories: All [P] tasks within each story can run in parallel
- Module Documentation: All tasks (T049-T054) can run in parallel
- Operational Guides: Both tasks (T056-T057) can run in parallel

### Parallel Opportunities

**After Foundational Phase Completes, Maximum Parallelization:**

```bash
# All user stories can proceed simultaneously:
Team Member 1: Phase 3 - US1 (Developer Onboarding) - T013-T017
Team Member 2: Phase 4 - US2 (AI-Assisted Development) - T018-T022
Team Member 3: Phase 5 - US6 (Architecture Understanding) - T023-T029
Team Member 4: Phase 6 - US3 (API Integration) - T030-T034
Team Member 5: Phase 7 - US4 (Configuration Management) - T035-T044
Team Member 6: Phase 8 - US5 (TSDB Backend Extension) - T045-T048
Team Member 7: Phase 9 - Module Documentation - T049-T055
Team Member 8: Phase 10 - Operational Guides - T056-T058
```

---

## Parallel Example: User Story 4 (Configuration Management)

```bash
# Launch all documentation pages for US4 together (different files):
Task T036: "Create doc/configuration/overview.md with configuration structure introduction"
Task T037: "Create doc/configuration/schema.md referencing auto-generated JSON schema"
Task T038: "Create doc/configuration/datasource.md documenting TSDB connection configuration"
Task T039: "Create doc/configuration/metric-templates.md documenting query templates"
Task T040: "Create doc/configuration/flag-metrics.md documenting flag metric configuration"
Task T041: "Create doc/configuration/health-metrics.md documenting health expressions"
Task T042: "Create doc/configuration/environments.md documenting environment configuration"

# Then complete the examples and integration tasks:
Task T043: "Create doc/configuration/examples.md with working configuration samples"
Task T044: "Add configuration section entries to doc/SUMMARY.md"
```

---

## Implementation Strategy

### MVP First (User Stories 1 & 2 Only)

1. Complete Phase 1: Setup (T001-T005)
2. Complete Phase 2: Foundational (T006-T012) - CRITICAL
3. Complete Phase 3: User Story 1 - Developer Onboarding (T013-T017)
4. Complete Phase 4: User Story 2 - AI-Assisted Development (T018-T022)
5. **STOP and VALIDATE**: Test that new developers can onboard in <30 min and AI tools can generate accurate code
6. Deploy documentation to team

**Why this MVP?**: US1 and US2 are both P1 priority and deliver immediate value to current team members and AI-powered development tools.

### Incremental Delivery

1. **Foundation** (Phases 1-2): Setup + Schemas → Tools ready
2. **MVP** (Phases 3-4): US1 + US2 → Developer onboarding + AI assistance
3. **Architecture** (Phase 5): US6 → Architecture understanding
4. **External Integration** (Phases 6-7): US3 + US4 → API + Configuration docs
5. **Extensibility** (Phase 8): US5 → TSDB backend extension guide
6. **Supporting Docs** (Phases 9-10): Modules + Operational guides
7. **Quality** (Phases 11-12): Validation + Polish

Each increment adds value without breaking previous deliverables.

### Parallel Team Strategy

With multiple developers after Foundational phase completion:

1. **Team completes Setup + Foundational together** (2-4 hours)
2. **Once Foundational is done, parallelize by user story:**
   - Developer A: US1 (Onboarding) - 2-3 hours
   - Developer B: US2 (AI Integration) - 2-3 hours
   - Developer C: US6 (Architecture) - 3-4 hours
   - Developer D: US3 (API) - 2-3 hours
   - Developer E: US4 (Configuration) - 4-5 hours
   - Developer F: US5 (TSDB Integration) - 2-3 hours
   - Developer G: Module Documentation - 3-4 hours
   - Developer H: Operational Guides - 2-3 hours
3. **Merge all stories** → Validation phase (1-2 hours)
4. **Polish together** (1-2 hours)

**Total Time**: 
- Sequential: 24-32 hours (1 developer)
- Parallel: 8-12 hours (8 developers)

---

## Task Summary

| Phase | Task Count | Can Parallelize | Est. Time (Solo) | Est. Time (Team) |
|-------|-----------|-----------------|------------------|------------------|
| Phase 1: Setup | 5 | 3 tasks | 1-2 hours | 30 min |
| Phase 2: Foundational | 7 | 4 tasks | 2-4 hours | 1-2 hours |
| Phase 3: US1 (Onboarding) | 5 | 2 tasks | 2-3 hours | 1-2 hours |
| Phase 4: US2 (AI) | 5 | 2 tasks | 2-3 hours | 1-2 hours |
| Phase 5: US6 (Architecture) | 7 | 3 tasks | 3-4 hours | 2-3 hours |
| Phase 6: US3 (API) | 5 | 2 tasks | 2-3 hours | 1-2 hours |
| Phase 7: US4 (Configuration) | 9 | 7 tasks | 4-5 hours | 2-3 hours |
| Phase 8: US5 (Integration) | 4 | 2 tasks | 2-3 hours | 1-2 hours |
| Phase 9: Module Docs | 7 | 6 tasks | 3-4 hours | 1-2 hours |
| Phase 10: Operational Guides | 3 | 2 tasks | 2-3 hours | 1-2 hours |
| Phase 11: Validation | 6 | 0 tasks | 1-2 hours | 1-2 hours |
| Phase 12: Polish | 7 | 0 tasks | 2-3 hours | 2-3 hours |
| **TOTAL** | **71 tasks** | **33 parallel** | **26-39 hours** | **15-24 hours** |

---

## Success Criteria Mapping

Tasks are designed to achieve all success criteria from spec.md:

- **SC-001** (30-min onboarding): Phase 3 (US1) - T014-T017
- **SC-002** (90% AI accuracy): Phase 4 (US2) - T018-T022
- **SC-003** (Zero source-code support): Phase 6 (US3) - T031-T034
- **SC-004** (80% first-attempt config): Phase 7 (US4) - T036-T044
- **SC-005** (<4 hour onboarding): Phases 3-10 combined
- **SC-006** (100% coverage): All phases 3-10
- **SC-007** (All examples work): Phase 11 (Validation) - T035, T059-T064
- **SC-008** (<3s search): Built-in mdbook search (no tasks needed)
- **SC-009** (Zero OpenAPI discrepancies): Phase 6 (US3) - T030
- **SC-010** (Accurate diagrams): Phase 5 (US6) - T024-T026 + T063
- **SC-011** (8-hour TSDB backend): Phase 8 (US5) - T045-T048
- **SC-012** (<2hr/month maintenance): Build automation via T006 + validation via T059-T064

---

## Notes

- **[P] marker**: Tasks that can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story] label**: Maps task to specific user story for traceability (US1, US2, US3, US4, US5, US6)
- **File paths**: All paths are absolute from repository root
- **Tests**: Validation tests included per research findings (not optional for documentation feature)
- **Checkpoints**: Each phase ends with a checkpoint to validate story independently
- **Format compliance**: All tasks follow strict checklist format: `- [ ] [TaskID] [P?] [Story?] Description with file path`

**Critical Path**: Setup → Foundational → {All User Stories in parallel} → Validation → Polish

**Minimum Viable Product**: Phases 1-4 (Setup + Foundational + US1 + US2) = Core onboarding + AI assistance
