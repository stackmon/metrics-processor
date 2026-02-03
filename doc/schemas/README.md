# Contracts: Machine-Readable Documentation Schemas

**Feature**: 001-project-documentation  
**Date**: 2025-01-23  
**Purpose**: Define machine-readable schemas for AI tools and IDEs

## Overview

This directory contains JSON Schema definitions and pattern documentation designed for consumption by AI-powered development tools, IDEs, and code generators.

---

## Schema Files

### config-schema.json

**Purpose**: Complete JSON Schema (Draft 7) for metrics-processor configuration files

**Usage**:
- **IDE Integration**: Provides autocomplete and validation in VSCode/IntelliJ when editing YAML config files
- **Runtime Validation**: Can be used with `jsonschema` crate to validate configuration at startup
- **AI Code Generation**: Enables LLMs to generate valid configuration examples

**Integration Example** (VSCode):
```json
// .vscode/settings.json
{
  "yaml.schemas": {
    "doc/schemas/config-schema.json": "config*.yaml"
  }
}
```

**Regenerating the Schema**:

The schema is generated from the actual Rust `Config` struct in `src/config.rs` using the `schemars` crate. To regenerate after config changes:

```bash
# Using make
make doc-schema

# Or using cargo directly
cargo test generate_config_schema -- --ignored --nocapture
```

This will update `doc/schemas/config-schema.json` with the latest schema.

**Validation Rules**:
- All required fields must be present (datasource, server, flag_metrics, health_metrics)
- Service names must use lowercase_with_underscores pattern
- Environment names must use lowercase-with-dashes pattern
- Flag metric templates must reference existing metric_template entries
- Health metric expressions must reference existing flag metrics

---

### patterns.json

**Purpose**: Document code patterns, conventions, and domain-specific logic not captured in type schemas

**Usage**:
- **AI Code Generation**: LLMs read this to understand project conventions and generate conformant code
- **Onboarding**: New developers reference to understand naming, error handling, and architectural patterns
- **IDE Plugins**: Can be parsed to provide context-aware suggestions

**Sections**:

1. **patterns[]**: Array of documented patterns with examples
   - Variable substitution in metric templates
   - Boolean expression evaluation in health metrics
   - Error handling conventions
   - Async/await usage patterns
   - Configuration validation approach
   - Logging conventions

2. **conventions**: Naming and structural conventions
   - File naming (Rust modules, docs, configs)
   - Data structure patterns
   - Testing organization

3. **architectural_principles**: High-level design decisions
   - Module separation of concerns
   - Binary responsibilities (convertor vs reporter)

**Example Usage** (AI prompt):
```
Given the patterns.json file, generate a new flag metric configuration
that monitors database query latency for the storage service.
```

---

## Implementation Notes

### Generation Strategy

These schemas should be **auto-generated** during the build process to ensure they stay synchronized with code:

```rust
// build.rs
use schemars::schema_for;
use cloudmon_metrics::config::Config;

fn main() {
    // Generate config-schema.json from Config struct
    let schema = schema_for!(Config);
    std::fs::write(
        "doc/schemas/config-schema.json",
        serde_json::to_string_pretty(&schema).unwrap()
    ).expect("Failed to write schema");
    
    println!("cargo:rerun-if-changed=src/config.rs");
}
```

### Validation Testing

Add tests to ensure schemas remain valid and examples conform:

```rust
// tests/schema_validation.rs
#[test]
fn config_schema_validates_examples() {
    let schema = include_str!("../doc/schemas/config-schema.json");
    let example = include_str!("../doc/configuration/examples.md");
    
    // Extract YAML blocks and validate against schema
    assert!(validate_yaml_against_schema(example, schema).is_ok());
}
```

---

## Maintenance

- **config-schema.json**: Auto-generated from `src/config.rs` - DO NOT EDIT MANUALLY
- **patterns.json**: Manually maintained - Update when conventions change
- **Version**: Both files should be versioned with project (currently 0.2.0)

---

## Target Consumers

| Consumer | File Used | Purpose |
|----------|-----------|---------|
| VSCode | config-schema.json | YAML autocomplete and validation |
| IntelliJ | config-schema.json | Configuration editing assistance |
| Claude/GPT | patterns.json | Understand code conventions for generation |
| GitHub Copilot | Both | Suggest conformant code and configuration |
| Custom tooling | config-schema.json | Runtime configuration validation |
| Documentation | Both | Generate reference documentation |

---

## Future Extensions

Potential additional schema files as project grows:

- **api-types-schema.json**: Request/response types for `/v1/health` endpoint
- **tsdb-interface-schema.json**: Contract for implementing new TSDB backends
- **plugin-schema.json**: If plugin system added for custom metrics

---

## References

- JSON Schema specification: https://json-schema.org/draft-07/schema
- `schemars` crate: https://docs.rs/schemars/latest/schemars/
- VSCode YAML extension: https://marketplace.visualstudio.com/items?itemName=redhat.vscode-yaml
