# manydns

A Rust library providing a provider-agnostic API for managing DNS zones and records.

## Overview

manydns defines abstract traits for DNS zone and record management, with concrete implementations for multiple DNS providers. The API design is inspired by the Go [libdns](https://github.com/libdns/libdns) project, adapting its conventions for Rust idioms while maintaining familiar semantics for developers coming from that ecosystem.

## Installation

Add `manydns` to your project with the provider you need:

```toml
[dependencies]
manydns = { version = "1.0", features = ["cloudflare"] }
```

## Quick Start

```rust
use manydns::{Provider, Zone, CreateRecord, DeleteRecord, RecordData};
use manydns::cloudflare::CloudflareProvider;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Create a provider with your API credentials
    let provider = CloudflareProvider::new("your_api_token")?;

    // Get a zone by domain name
    let zone = provider.get_zone("example.com").await?;
    println!("Zone: {} (ID: {})", zone.domain(), zone.id());

    // List all records in the zone
    let records = zone.list_records().await?;
    for record in &records {
        println!("  {} {} -> {:?}", record.host, record.data.get_type(), record.data);
    }

    // Create a new A record
    let record = zone.create_record(
        "www",                                    // host (use "@" for apex)
        &RecordData::A("192.0.2.1".parse()?),    // record data
        300,                                      // TTL in seconds
    ).await?;
    println!("Created record: {}", record.id);

    // Delete a record by ID
    zone.delete_record(&record.id).await?;

    Ok(())
}
```

## Supported Providers

Enable providers via feature flags:

| Provider | Feature Flag | Zone Create/Delete |
|----------|--------------|-------------------|
| [Cloudflare](https://www.cloudflare.com/) | `cloudflare` | No |
| [Hetzner DNS](https://www.hetzner.com/dns-console/) | `hetzner` | Yes |
| [DNSPod](https://www.dnspod.cn/) | `dnspod` | No |
| [Tencent Cloud](https://cloud.tencent.com/) | `tencent` | No |
| [Technitium](https://technitium.com/dns/) | `technitium-dns` | Yes |
| [Namecheap](https://www.namecheap.com/) | `namecheap` | No |

## Core Traits

The library uses a capability-based trait design:

```rust
// Provider: entry point for zone access
pub trait Provider {
    type Zone: Zone;
    async fn list_zones(&self) -> Result<Vec<Self::Zone>, ...>;
    async fn get_zone(&self, zone_id: &str) -> Result<Self::Zone, ...>;
}

// Zone: record management within a zone
pub trait Zone {
    fn id(&self) -> &str;
    fn domain(&self) -> &str;
    async fn list_records(&self) -> Result<Vec<Record>, ...>;
    async fn get_record(&self, record_id: &str) -> Result<Record, ...>;
}

// Optional capabilities
pub trait CreateRecord: Zone { ... }
pub trait DeleteRecord: Zone { ... }
pub trait CreateZone: Provider { ... }
pub trait DeleteZone: Provider { ... }
```

## Record Types

Supported DNS record types:

- **A** - IPv4 address
- **AAAA** - IPv6 address
- **CNAME** - Canonical name (alias)
- **MX** - Mail exchange
- **NS** - Name server
- **TXT** - Text record
- **SRV** - Service record
- **Other** - Pass-through for provider-specific types

## Record Naming Convention

Record names are **relative to the zone**, following the same convention as Go libdns:

- `"www"` refers to `www.example.com` in zone `example.com`
- `"@"` refers to the zone apex (`example.com` itself)
- `"sub.domain"` refers to `sub.domain.example.com`

## TLS Backend

Provider implementations use [reqwest](https://crates.io/crates/reqwest) for HTTP. By default, `default-tls` is enabled. Alternative backends:

```toml
# Use rustls instead of native TLS
manydns = { version = "1.0", default-features = false, features = ["rustls-tls", "cloudflare"] }
```

Available: `default-tls` (default), `rustls-tls`, `native-tls`, `native-tls-vendored`

## Comparison with Go libdns

This library adapts the Go libdns API for Rust:

| Go libdns | Rust manydns |
|-----------|--------------|
| `provider.GetRecords(ctx, zone)` | `provider.get_zone(zone).await?.list_records().await?` |
| Zone as string parameter | Zone as trait object |
| Batch operations on `[]Record` | Single record operations |
| Delete by record content match | Delete by provider-specific ID |

Record naming conventions (`@` for apex, relative names) are intentionally identical.

## Contributing

Contributions welcome! See the existing provider implementations for patterns to follow when adding new providers.

## License

0BSD
