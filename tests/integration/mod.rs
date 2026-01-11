//! Integration tests for DNS providers.
//!
//! These tests require valid provider credentials and are ignored by default.
//! Each provider has its own submodule with specific setup instructions.
//!
//! # Running Tests
//!
//! 1. Create a `.env` file in the project root (see `.env.example`)
//! 2. Run with: `cargo test --features <provider> -- --ignored`
//!
//! # Available Provider Tests
//!
//! - `cloudflare`: Cloudflare DNS API (using API token)
//! - `dnspod`: DNSPod legacy API (using API tokens)
//! - `tencent`: Tencent Cloud DNSPod API (using SecretId/SecretKey)

#[cfg(feature = "cloudflare")]
mod cloudflare;

#[cfg(feature = "dnspod")]
mod dnspod;

#[cfg(feature = "tencent")]
mod tencent;

#[cfg(feature = "namecheap")]
mod namecheap;
