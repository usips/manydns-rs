//! Mock-based tests for DNS providers.
//!
//! These tests use wiremock to simulate API responses without hitting real APIs.
//! Each provider has its own test module with comprehensive coverage of the API.

#[cfg(feature = "cloudflare")]
pub mod cloudflare;

#[cfg(feature = "hetzner")]
pub mod hetzner;

#[cfg(feature = "namecheap")]
pub mod namecheap;
