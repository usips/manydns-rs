//! Test suite entry point.
//!
//! This file serves as the entry point for all test modules.
//!
//! # Test Organization
//!
//! - `common/` - Shared test utilities and mock helpers
//! - `unit/` - Unit tests for internal types and helper functions
//! - `mock/` - Mock-based tests using wiremock (no network required)
//! - `integration/` - Live integration tests (require credentials)
//!
//! # Running Tests
//!
//! ```bash
//! # Run all non-integration tests
//! cargo test
//!
//! # Run mock tests for a specific provider
//! cargo test --features cloudflare
//!
//! # Run integration tests (require credentials in .env)
//! cargo test --features cloudflare -- --ignored
//! ```

// Shared test utilities
mod common;

// Unit tests for internal types and helpers
mod unit;

// Mock-based tests (no network required)
mod mock;

// Integration tests (require live API credentials)
mod integration;
