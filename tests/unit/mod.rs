//! Unit tests for core library types and internal functions.
//!
//! These tests focus on internal types, parsing logic, and helper functions
//! that don't require network access or mock servers.

mod types;

#[cfg(feature = "namecheap")]
mod namecheap;

#[cfg(feature = "cloudflare")]
mod http_config;
