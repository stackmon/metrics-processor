# Research: Comprehensive Project Documentation

**Feature**: 001-project-documentation  
**Date**: 2025-01-23  
**Status**: Complete

## Overview

This document consolidates research findings for implementing comprehensive project documentation that serves both human developers and AI-powered tools. Research focused on four key areas: documentation validation, diagram tooling, AI-friendly schemas, and mdbook best practices.

---

## 1. Documentation Validation & Testing

### Decision: Multi-layered validation approach

**Rationale**: Edge cases require ensuring documentation examples remain correct as code evolves. Rust ecosystem provides robust tooling for this.

### Tools Selected

| Validation Type | Tool | Purpose |
|----------------|------|---------|
| Code examples in docs | `cargo test --doc` | Validates Rust code blocks compile and run |
| YAML config examples | Custom test harness with `serde_yaml` | Ensures config examples parse correctly |
| OpenAPI sync | `utoipa` crate + test assertions | Auto-generate schema from code, validate against openapi-schema.yaml |
| Markdown links | `mdbook-linkcheck` plugin | Validates all internal/external links |

### Implementation Pattern

```rust
// tests/documentation_validation.rs
#[test]
fn validate_config_examples() {
    let config_example = include_str!("../doc/configuration/examples.md");
    // Extract YAML blocks from markdown
    let yaml_blocks = extract_yaml_from_markdown(config_example);
    
    for (i, yaml) in yaml_blocks.iter().enumerate() {
        let parsed: Result<Config, _> = serde_yaml::from_str(yaml);
        assert!(parsed.is_ok(), "Example {} failed to parse", i);
    }
}
```

### Alternatives Considered

- **Manual review**: Rejected due to high maintenance burden and human error risk
- **External validation services**: Rejected due to lack of Rust-specific tooling
- **CI-only validation**: Rejected - want pre-commit hooks to catch issues early

---

## 2. Diagram Generation & Tooling

### Decision: Mermaid diagrams for all architecture and data flow visualizations

**Rationale**: Text-based format enables version control, AI parsing (FR-013), and browser rendering. Superior to binary formats (SVG/PNG) for maintainability.

### Selected Format: Mermaid.js

**Pros:**
- ✅ Text-based → Git diffs show semantic changes
- ✅ AI-parseable → LLMs can understand diagram structure
- ✅ Native browser rendering via `mdbook-mermaid` plugin
- ✅ No build step → Write in markdown, renders automatically
- ✅ Version control friendly → Merge conflicts are rare and readable

**Cons:**
- Limited fine-grained styling (acceptable tradeoff)
- Complex diagrams become verbose (mitigated by splitting into multiple diagrams)

### Plugin Configuration

```toml
# doc/book.toml
[preprocessor.mermaid]
command = "mdbook-mermaid"

[output.html]
additional-css = ["theme/mermaid.css"]
```

### Diagram Types Required

| Diagram Type | Mermaid Type | Purpose |
|-------------|-------------|---------|
| Architecture Overview | `graph TB` | Show convertor, reporter, TSDB, dashboard relationships (FR-006) |
| Data Flow | `sequenceDiagram` | Show TSDB → flag metrics → health metrics flow (FR-005) |
| Configuration Structure | `graph LR` | Show configuration section relationships (FR-004) |
| Module Dependencies | `graph TD` | Show Rust module structure (FR-009) |

### Alternatives Considered

- **PlantUML**: Rejected - requires external server for rendering, less AI-friendly syntax
- **GraphViz**: Rejected - steeper learning curve, less intuitive for team
- **SVG/PNG**: Rejected - binary format breaks version control and AI parsing

---

## 3. AI-Friendly Schema Generation

### Decision: `schemars` crate + build.rs script for automatic JSON Schema generation

**Rationale**: Enables IDE autocomplete and AI code generation (User Story 2) while maintaining single source of truth in Rust structs.

### Implementation Architecture

```
Rust structs (src/config.rs)
    ↓ [schemars derive]
JSON Schema (doc/schemas/config-schema.json)
    ↓ [IDE reads]
Autocomplete in VSCode/IntelliJ
    ↓ [AI tools read]
Code generation suggestions
```

### Tools Selected

| Use Case | Tool | Format |
|---------|------|--------|
| Config struct → JSON Schema | `schemars` crate | JSON Schema Draft 7 |
| Runtime validation | `jsonschema` crate | Validates YAML against schema |
| Type exports for TypeScript | `ts-rs` crate (optional) | TypeScript interfaces |
| IDE integration | `.vscode/settings.json` | VSCode JSON schema mapping |

### Generated Schema Structure

```json
// doc/schemas/config-schema.json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "CloudMon Metrics Configuration",
  "type": "object",
  "required": ["datasource", "server", "flag_metrics", "health_metrics"],
  "properties": {
    "datasource": {
      "type": "object",
      "properties": {
        "url": {"type": "string", "format": "uri"},
        "type": {"type": "string", "enum": ["graphite"]}
      }
    }
    // ... rest generated from Config struct
  }
}
```

### Patterns Documentation for AI

Created `doc/schemas/patterns.json` for conventions not captured in schemas:

```json
{
  "patterns": [
    {
      "name": "metric_template_variable_substitution",
      "syntax": "$variable or ${variable}",
      "available_variables": ["service", "environment"],
      "example": "stats.timers.$service.$environment.mean"
    },
    {
      "name": "health_expression_syntax",
      "operators": ["||", "&&", "!"],
      "operands": "service.metric_name references",
      "example": "api.slow || api.success_rate_low"
    }
  ],
  "conventions": {
    "naming": {
      "services": "lowercase_with_underscores",
      "environments": "lowercase-with-dashes",
      "metrics": "service.metric_name format"
    }
  }
}
```

### Alternatives Considered

- **Manual JSON Schema writing**: Rejected - high maintenance burden, prone to drift from code
- **OpenAPI only**: Rejected - doesn't cover configuration, only API
- **TypeScript-first approach**: Rejected - Rust is source of truth

---

## 4. mdbook Configuration & Best Practices

### Decision: Enhanced mdbook with search, mermaid, and collapsible sections

**Rationale**: Existing `doc/` infrastructure works well. Enhance rather than replace to minimize migration effort.

### Configuration Enhancements

```toml
# doc/book.toml (enhancements)
[book]
title = "CloudMon Metrics Processor"
authors = ["CloudMon Team"]
language = "en"
multilingual = false
src = "."

[preprocessor.mermaid]
command = "mdbook-mermaid"

[preprocessor.linkcheck]
# Validates all links

[output.html]
default-theme = "light"
preferred-dark-theme = "navy"
git-repository-url = "https://github.com/your-org/metrics-processor"

[output.html.search]
enable = true
limit-results = 30
use-boolean-and = true

[output.html.fold]
enable = true  # Collapsible sidebar sections
level = 1      # Fold chapters by default
```

### Documentation Structure

```
doc/
├── SUMMARY.md               # Navigation (enhanced)
├── index.md                 # Project overview (enhanced per FR-001)
├── architecture/            # NEW
│   ├── overview.md
│   ├── diagrams.md          # Mermaid diagrams
│   └── data-flow.md
├── getting-started/         # NEW
│   ├── quickstart.md
│   ├── project-structure.md
│   └── development.md
├── api/                     # NEW
│   ├── endpoints.md
│   ├── authentication.md
│   └── examples.md
├── configuration/           # ENHANCED (from config.md)
│   ├── overview.md
│   ├── schema.md
│   ├── datasource.md
│   ├── metric-templates.md
│   ├── flag-metrics.md
│   ├── health-metrics.md
│   ├── environments.md
│   └── examples.md
├── components/              # ENHANCED (from convertor.md, reporter.md)
│   ├── convertor.md
│   └── reporter.md
├── integration/             # NEW
│   ├── interface.md
│   ├── graphite.md
│   └── adding-backends.md
├── modules/                 # NEW
│   ├── overview.md
│   ├── api.md
│   ├── config.md
│   ├── types.md
│   ├── graphite.md
│   └── common.md
└── guides/                  # NEW
    ├── troubleshooting.md
    └── deployment.md
```

### Search Performance (SC-008)

Built-in mdbook search achieves <1s response time for typical queries:
- Index size: ~200KB for 50 pages
- JavaScript-based client-side search
- No backend required

### Versioning Strategy

**Current**: Single version (0.2.0)  
**Future**: Use Git tags + GitHub Pages branches when needed  
**Not needed now**: Project is pre-1.0, breaking changes are acceptable

### Alternatives Considered

- **Docusaurus**: Rejected - requires Node.js ecosystem, heavier setup
- **Sphinx**: Rejected - Python-based, less natural for Rust projects
- **Custom static site**: Rejected - high maintenance burden
- **rustdoc only**: Rejected - not suitable for user guides and architecture docs

---

## 5. Tooling Recommendations Summary

### Development Tools

```toml
# Cargo.toml additions
[dependencies]
schemars = "0.8"

[build-dependencies]
schemars = "0.8"

[dev-dependencies]
# (existing mockito, tempfile already suitable)
```

### Documentation Tools

```bash
# Install once
cargo install mdbook
cargo install mdbook-mermaid
cargo install mdbook-linkcheck

# Build documentation
mdbook build doc/
mdbook serve doc/  # Local preview at http://localhost:3000
```

### CI/CD Integration

```yaml
# .github/workflows/docs.yml (or Zuul equivalent)
- name: Validate documentation
  run: |
    cargo test --doc
    cargo test --test documentation_validation
    mdbook build doc/
    # Check for broken links
    mdbook test doc/
```

### Pre-commit Hooks

```yaml
# .pre-commit-config.yaml additions
- repo: local
  hooks:
    - id: mdbook-test
      name: Validate documentation examples
      entry: cargo test --test documentation_validation
      language: system
      pass_filenames: false
```

---

## 6. Implementation Phases

### Phase 0: Research (COMPLETE)
✅ Validated tooling choices  
✅ Identified implementation patterns  
✅ Resolved technical unknowns

### Phase 1: Design & Contracts (NEXT)
- Create data-model.md defining documentation structure entities
- Generate JSON schemas in contracts/ directory
- Write quickstart.md for developers
- Update agent context

### Phase 2: Implementation (Future - via /speckit.tasks)
- Enhance mdbook.toml with plugins
- Create new documentation sections
- Generate schemas with build.rs
- Write mermaid diagrams
- Migrate existing docs
- Add validation tests

---

## Decisions Log

| Decision | Rationale | Risk Mitigation |
|---------|----------|-----------------|
| Mermaid over PlantUML | Version control + AI parsing | Document complex diagram patterns |
| schemars for schema gen | Single source of truth | Add validation tests |
| mdbook enhancement | Preserve existing work | Incremental migration |
| cargo test for validation | Native Rust tooling | Add pre-commit hooks |
| JSON Schema Draft 7 | Wide IDE support | Document VSCode setup |

---

## Open Questions (None Remaining)

All technical unknowns resolved. Ready to proceed to Phase 1: Design.
