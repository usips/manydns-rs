# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

manydns is a Rust library providing a provider-agnostic API for managing DNS zones and records. The API design is inspired by the Go [libdns](https://github.com/libdns/libdns) project, maintaining a familiar interface. The crate defines core traits and optionally includes implementations for multiple DNS providers.

## Build and Test Commands

```bash
# Build with all provider features
cargo build --all-features

# Run all non-integration tests
cargo test --all-features

# Run tests for a specific provider
cargo test --features cloudflare

# Run a single test
cargo test --all-features test_name

# Run integration tests (require credentials in .env)
cargo test --features cloudflare -- --ignored

# Linting and formatting
cargo fmt
cargo clippy --all-features

# Fuzzing (requires nightly)
cargo +nightly fuzz run fuzz_label
```

## Architecture

### Core Trait Hierarchy

The library uses a capability-based trait design:

**Provider traits** (`src/lib.rs`):
- `Provider` - Base trait for zone retrieval (`list_zones`, `get_zone`)
- `CreateZone` - Optional capability for zone creation
- `DeleteZone` - Optional capability for zone deletion

**Zone traits** (`src/lib.rs`):
- `Zone` - Base trait for record retrieval (`list_records`, `get_record`)
- `CreateRecord` - Optional capability for record creation
- `DeleteRecord` - Optional capability for record deletion

Each trait has an associated `Custom*Error` type allowing providers to extend the standard error variants.

### Provider Implementation Pattern

Each provider lives in its own module (e.g., `src/cloudflare/`) with:
- `mod.rs` - Implements `Provider`, `Zone`, and capability traits
- `api.rs` - HTTP client for the provider's API

Providers wrap their API client in `Arc` for shared ownership across zones.

### Types Module

`src/types.rs` contains RFC-compliant fixed-size DNS types:
- `Label` (64 bytes) - Single DNS label, max 63 characters
- `DomainName` (256 bytes) - Full domain name, max 255 characters
- `Ttl` - Time-to-live with RFC 2181 max (2^31 - 1)
- `RecordType` - DNS record type enum

### Feature Flags

Providers are feature-gated:
- `cloudflare`, `hetzner`, `dnspod`, `tencent`, `technitium-dns`, `namecheap`

TLS backends (for reqwest):
- `default-tls` (default), `rustls-tls`, `native-tls`, `native-tls-vendored`

## Test Organization

Tests are in `tests/` with this structure:
- `unit/` - Tests for internal types (Label, DomainName, Ttl, RecordType)
- `mock/` - Wiremock-based tests simulating API responses (no network)
- `integration/` - Live API tests marked `#[ignore]`, require `.env` credentials
- `common/` - Shared test utilities and mock helpers

## Key Conventions

- All I/O methods return `impl Future<Output = Result<..>>` (async-first)
- Use `thiserror` for error types with `Custom(T)` variants
- Tests use `#[tokio::test]` for async execution
- Property-based testing with `proptest` for type validation
- Provider-specific quirks documented in rustdoc (see Namecheap's destructive update warning)
