//! Cloudmon-metrics - convert plain TSDB metrics into semaphore like values
//!
//! When monitoring a cloud it is usual to have variety of metrics of different types (like latency
//! of API calls, success rates, etc).
pub mod api;
pub mod common;
pub mod config;
pub mod graphite;
pub mod types;
