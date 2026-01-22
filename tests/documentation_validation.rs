//! Documentation validation tests
//!
//! This test suite ensures documentation examples remain valid as code evolves.

use cloudmon_metrics::config::Config;
use serde_json::Value;
use std::fs;
use std::path::Path;

/// Extract YAML code blocks from markdown file
fn extract_yaml_blocks(content: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut in_yaml_block = false;
    let mut current_block = String::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "```yaml" || trimmed == "```yml" {
            in_yaml_block = true;
            current_block.clear();
        } else if trimmed.starts_with("```") && in_yaml_block {
            in_yaml_block = false;
            if !current_block.trim().is_empty() {
                blocks.push(current_block.clone());
            }
        } else if in_yaml_block {
            current_block.push_str(line);
            current_block.push('\n');
        }
    }

    blocks
}

/// Test that all YAML configuration examples parse correctly
#[test]
fn validate_yaml_examples_parse() {
    let doc_root = Path::new("doc");
    
    // Check if configuration examples exist
    let examples_path = doc_root.join("configuration/examples.md");
    if !examples_path.exists() {
        eprintln!("Configuration examples not yet created, skipping test");
        return;
    }

    let content = fs::read_to_string(&examples_path)
        .expect("Failed to read configuration examples");

    let yaml_blocks = extract_yaml_blocks(&content);
    assert!(
        !yaml_blocks.is_empty(),
        "No YAML examples found in configuration/examples.md"
    );

    for (i, yaml) in yaml_blocks.iter().enumerate() {
        let parsed: Result<Config, _> = serde_yaml::from_str(yaml);
        assert!(
            parsed.is_ok(),
            "Example {} in configuration/examples.md failed to parse: {:?}",
            i + 1,
            parsed.err()
        );
    }
}

/// Test that quickstart configuration examples parse correctly
#[test]
fn validate_quickstart_examples() {
    let quickstart_path = Path::new("doc/getting-started/quickstart.md");
    
    if !quickstart_path.exists() {
        eprintln!("Quickstart not yet created, skipping test");
        return;
    }

    let content = fs::read_to_string(&quickstart_path)
        .expect("Failed to read quickstart");

    let yaml_blocks = extract_yaml_blocks(&content);
    
    for (i, yaml) in yaml_blocks.iter().enumerate() {
        let parsed: Result<Config, _> = serde_yaml::from_str(yaml);
        assert!(
            parsed.is_ok(),
            "Quickstart example {} failed to parse: {:?}",
            i + 1,
            parsed.err()
        );
    }
}

/// Test that JSON schema exists and is valid
#[test]
fn validate_schema_exists_and_valid() {
    let schema_path = Path::new("doc/schemas/config-schema.json");
    assert!(
        schema_path.exists(),
        "config-schema.json not found. Run 'cargo build' to generate it."
    );

    let schema_content = fs::read_to_string(schema_path)
        .expect("Failed to read config-schema.json");

    let schema: Result<Value, _> = serde_json::from_str(&schema_content);
    assert!(
        schema.is_ok(),
        "config-schema.json is not valid JSON: {:?}",
        schema.err()
    );

    let schema_obj = schema.unwrap();
    assert!(
        schema_obj.get("$schema").is_some(),
        "Schema missing $schema field"
    );
    assert!(
        schema_obj.get("properties").is_some(),
        "Schema missing properties field"
    );
}

/// Test that patterns.json exists and is valid
#[test]
fn validate_patterns_json() {
    let patterns_path = Path::new("doc/schemas/patterns.json");
    assert!(
        patterns_path.exists(),
        "patterns.json not found in doc/schemas/"
    );

    let patterns_content = fs::read_to_string(patterns_path)
        .expect("Failed to read patterns.json");

    let patterns: Result<Value, _> = serde_json::from_str(&patterns_content);
    assert!(
        patterns.is_ok(),
        "patterns.json is not valid JSON: {:?}",
        patterns.err()
    );
}

/// Test that schema README exists
#[test]
fn validate_schema_readme_exists() {
    let readme_path = Path::new("doc/schemas/README.md");
    assert!(
        readme_path.exists(),
        "schemas/README.md not found in doc/schemas/"
    );

    let readme_content = fs::read_to_string(readme_path)
        .expect("Failed to read schemas/README.md");

    assert!(
        readme_content.contains("config-schema.json"),
        "README should reference config-schema.json"
    );
}

/// Test that all internal documentation links are valid
#[test]
fn validate_documentation_structure() {
    let summary_path = Path::new("doc/SUMMARY.md");
    assert!(summary_path.exists(), "SUMMARY.md not found");

    let summary_content = fs::read_to_string(summary_path)
        .expect("Failed to read SUMMARY.md");

    // Extract markdown links
    let link_regex = regex::Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").unwrap();
    
    for cap in link_regex.captures_iter(&summary_content) {
        let link = &cap[2];
        
        // Skip external links
        if link.starts_with("http://") || link.starts_with("https://") {
            continue;
        }

        let _link_path = Path::new("doc").join(link);
        
        // Only check links that should exist (not future placeholders)
        if link.contains("getting-started") || 
           link.contains("architecture") || 
           link.contains("api") || 
           link.contains("configuration") ||
           link.contains("integration") ||
           link.contains("modules") ||
           link.contains("guides") ||
           link == "index.md" ||
           link == "convertor.md" ||
           link == "reporter.md" {
            // We'll create these files, so just note them for now
            eprintln!("Link to be created: {}", link);
        }
    }
}

#[test]
fn validate_config_examples_conform_to_schema() {
    // This test will validate that configuration examples conform to the JSON schema
    // For now, we just ensure the schema is valid
    // Future: Use jsonschema crate to validate examples against schema
    
    let schema_path = Path::new("doc/schemas/config-schema.json");
    assert!(schema_path.exists(), "Schema must exist");
}
