# Feature Specification: Reporter Migration to Status Dashboard API V2

**Feature Branch**: `003-sd-api-v2-migration`  
**Created**: 2025-01-22  
**Status**: Draft  
**Input**: User description: "Migrate the reporter from Status Dashboard API V1 to V2 for sending incidents"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Reporter Creates Incidents via V2 API (Priority: P1)

The reporter monitors service health metrics and automatically creates incidents in the Status Dashboard when issues are detected. After migration, the reporter must successfully create incidents using the new V2 API endpoint while maintaining the same monitoring capabilities.

**Why this priority**: This is the core functionality of the reporter. Without this working, no incidents can be reported to the Status Dashboard, making the entire monitoring system ineffective.

**Independent Test**: Can be fully tested by triggering a service health issue (impact value > 0) and verifying that an incident appears in the Status Dashboard with the correct component ID, impact level, and timestamp. Delivers the fundamental value of automated incident reporting.

**Acceptance Scenarios**:

1. **Given** the reporter detects a service health issue with impact > 0, **When** it sends an incident to the Status Dashboard, **Then** the incident is created successfully via the `/v2/incidents` endpoint with component ID, title, description, impact, start_date, system flag, and type fields.

2. **Given** the reporter has a valid component name and attributes from config, **When** it needs to report an incident, **Then** it successfully resolves the component name to a component ID by querying the components cache.

3. **Given** multiple services are being monitored, **When** issues are detected in different services, **Then** each incident is created with the correct component ID matching the service's component configuration.

---

### User Story 2 - Component Cache Management (Priority: P2)

The reporter maintains a cache mapping component names and attributes to component IDs to avoid repeated lookups. When a component is not found in the cache, the reporter refreshes the cache from the Status Dashboard API.

**Why this priority**: This enables efficient operation and handles cases where new components are added to the Status Dashboard after the reporter starts. Without this, the reporter would fail when encountering unknown components.

**Independent Test**: Can be tested by starting the reporter, adding a new component to the Status Dashboard, triggering an issue for that component, and verifying the reporter refreshes the cache and successfully creates the incident.

**Acceptance Scenarios**:

1. **Given** the reporter starts up, **When** initialization occurs, **Then** the reporter fetches all components from `/v2/components` endpoint and builds a component ID cache.

2. **Given** a component is not found in the cache, **When** the reporter needs to report an incident, **Then** it refreshes the cache from the API and retries the component lookup.

3. **Given** the initial cache load fails, **When** the reporter starts, **Then** it retries fetching components up to 3 times with 60-second delays before giving up.

---

### User Story 3 - Authorization Remains Unchanged (Priority: P3)

The reporter continues to use the same authorization mechanism (HMAC-based JWT token) for authenticating with the Status Dashboard API, ensuring no changes to security configuration are required.

**Why this priority**: Maintaining existing authorization reduces migration complexity and avoids requiring configuration changes or credential updates during the migration.

**Independent Test**: Can be tested by verifying that the reporter uses the existing secret from config to generate the JWT token and successfully authenticates with the V2 endpoints using the same Authorization header format as V1.

**Acceptance Scenarios**:

1. **Given** the reporter has a configured secret, **When** it makes requests to V2 endpoints, **Then** it includes the same HMAC-signed JWT token in the Authorization header as used with V1.

2. **Given** no secret is configured, **When** the reporter starts, **Then** it operates without authentication headers (for environments without auth requirements).

---

### Edge Cases

- What happens when the Status Dashboard API is unavailable during initial cache load?
  - Reporter should retry up to 3 times with delays, then fail to start with clear error message
  
- What happens when a component name exists but with different attributes than configured?
  - Reporter should match components where the configured attributes are a subset of the component's attributes
  
- What happens when the API returns an error during incident creation?
  - Reporter should log the error with the response status and body, continue without retry, and rely on the next monitoring cycle (typically ~5 minutes) to re-attempt incident creation
  
- What happens when multiple components match the same name and attributes?
  - Reporter should use the first matching component ID found in the cache
  
- What happens when the component cache refresh fails?
  - Reporter should log a warning, continue using the old cache, and report that the component was not found

- What happens when the service health response contains no datapoints?
  - Reporter should skip incident creation and continue to the next service check

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Reporter MUST send incident data to the `/v2/incidents` endpoint instead of `/v1/component_status`

- **FR-002**: Reporter MUST use the new incident data structure containing: title (static value "System incident from monitoring system"), description (empty string), impact (0=none, 1=minor, 2=major, 3=critical, derived directly from service health expression weight), components (array of component IDs), start_date, system flag, and type

- **FR-003**: Reporter MUST fetch components from `/v2/components` endpoint at startup and build a cache mapping (component name, attributes) to component ID

- **FR-004**: Reporter MUST resolve component names to component IDs using the cache before creating incidents

- **FR-005**: Reporter MUST refresh the component cache when a component is not found and retry the lookup once

- **FR-006**: Reporter MUST retry the initial component cache load up to 3 times with 60-second delays between attempts

- **FR-007**: Reporter MUST fail to start if the initial component cache load fails after all retry attempts

- **FR-008**: Reporter MUST continue using the existing HMAC-signed JWT authorization mechanism without changes

- **FR-009**: Reporter MUST include the system flag set to true in incident data to indicate automatic creation

- **FR-010**: Reporter MUST set the incident type to "incident" for all automatically created incidents

- **FR-011**: Reporter MUST use the timestamp from the health metric as the start_date, adjusted by -1 second to align with monitoring intervals

- **FR-012**: Reporter MUST match components where the configured attributes are a subset of the component's attributes in the Status Dashboard

- **FR-013**: Reporter MUST log comprehensive incident information including timestamp, status, service, environment, component details, and triggered metrics

- **FR-014**: Reporter MUST increase the HTTP timeout from 2 seconds to 10 seconds to accommodate the new endpoint's response times

- **FR-015**: Reporter MUST continue monitoring other services even if incident creation fails for one service, logging the error without immediate retry and allowing the next monitoring cycle to re-attempt

- **FR-016**: Reporter MUST create a new incident request for every service health issue detection, relying on the Status Dashboard's built-in duplicate handling to return existing incidents when applicable

- **FR-017**: Reporter MUST log structured diagnostic details containing: detection timestamp, service name, environment name, component name and attributes, impact value, and a list of all triggered metric names with values that contributed to the easest health issue detection

### Key Entities

- **Incident (V2)**: Represents an incident in the Status Dashboard V2 API with fields: title (string, static value "System incident from monitoring system"), description (string, MUST be empty string), impact (integer 0-3 where 0=none, 1=minor, 2=major, 3=critical), components (array of component IDs), start_date (RFC3339 datetime), system (boolean), type (enum: "incident", "maintenance", "info"). The impact value is derived directly from the service health expression weight field. Diagnostic details (timestamp, service name, environment, component details, impact value, triggered metrics) MUST be logged for operational purposes.

- **Component (V2)**: Represents a component in Status Dashboard with fields: id (integer), name (string), attributes (array of name-value pairs). Used to resolve component names to IDs.

- **Component Cache**: In-memory mapping from (component name, sorted attributes) to component ID, used to avoid repeated API calls for component resolution

- **Service Health Point**: Enhanced health metric data containing: timestamp, impact value, list of triggered metric names, and optional metric value for detailed logging

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Reporter successfully creates incidents in the Status Dashboard using the V2 API within 10 seconds of detecting a service health issue

- **SC-002**: Reporter resolves component names to IDs without errors for 100% of configured components that exist in the Status Dashboard

- **SC-003**: Reporter automatically recovers from missing component errors by refreshing the cache within one monitoring cycle (approximately 5 minutes)

- **SC-004**: Reporter starts successfully within 3 minutes even when the Status Dashboard API is slow, thanks to retry logic

- **SC-005**: All automatically created incidents are correctly tagged with system=true and type="incident" in the Status Dashboard

- **SC-006**: Reporter logs provide sufficient information to troubleshoot incident creation failures, including component names, attributes, and API responses

## Dependencies

- **Status Dashboard API V2**: The Status Dashboard must have the `/v2/incidents` and `/v2/components` endpoints available and functional
- **Backward Compatibility**: The migration does not require changes to the reporter configuration file format or authorization mechanism
- **Component Registration**: All monitored components must be registered in the Status Dashboard with matching names and attributes

## Assumptions

- The Status Dashboard API V2 is stable and ready for production use
- Component IDs in the Status Dashboard are stable and do not change frequently
- The authorization mechanism (HMAC-signed JWT) is compatible with both V1 and V2 endpoints
- The reporter's monitoring logic and configuration structure remain unchanged
- The Status Dashboard will accept incidents with system=true flag for automatically generated incidents
- Component matching logic (subset attribute matching) is sufficient for all use cases
- The 10-second HTTP timeout is sufficient for the V2 API response times under normal operation

## Clarifications

### Session 2025-01-22

- Q: How should the reporter handle duplicate incident detection events within the same monitoring cycle? → A: Option A - Create incidents on every detection. The Status Dashboard ignores duplicate requests and returns the existing event, so no client-side deduplication needed.
- Q: What should the error recovery strategy be when incident creation fails? → A: Option B - Log the error and continue without retry, rely on next monitoring cycle.
- Q: How should the reporter map service health impact values to incident impact values? → A: Option B - Use service health "impact" field directly (0=none, 1=minor, 2=major, 3=critical). The current V1 implementation already passes the health expression weight directly as the impact value.
- Q: What format should the incident title use? → A: Use a generic static title "System incident from monitoring system" (as implemented in sd_api_v2_migration branch).

### Session 2026-01-22

- Q: What content should be included in the incident description field? → A: Empty string.

## Out of Scope

- Changes to the monitoring logic or health metric evaluation
- Modifications to the reporter configuration file format
- Updates to the authorization mechanism or secret management
- Migration of existing V1 incidents to V2 format
- Support for additional incident types beyond "incident" (e.g., "maintenance", "info")
- Batch incident creation or update operations
- Incident updates or closure operations (only creation is in scope)
- Changes to the component attribute configuration format
- Performance optimizations beyond the timeout adjustment
- Automatic component creation in the Status Dashboard if not found
