// Shared test fixtures and utilities for integration tests
//
// This module provides:
// - Configuration fixtures (configs.rs)
// - Graphite mock response data (graphite_responses.rs)
// - Test helper functions and custom assertions (helpers.rs)
//
// Each integration test file is compiled as a separate crate, so not all
// fixtures are used in every test file. #[allow(dead_code)] suppresses
// warnings for items not used in a particular test compilation.

#[allow(dead_code)]
pub mod configs;
#[allow(dead_code)]
pub mod graphite_responses;
#[allow(dead_code)]
pub mod helpers;
