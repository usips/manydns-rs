# manydns

A Rust library providing a provider-agnostic API for managing DNS zones and records.

## Overview

manydns defines abstract traits for DNS zone and record management, with concrete implementations for multiple DNS providers. The API design is inspired by the Go [libdns](https://github.com/libdns/libdns) project, maintaining a familiar interface for developers coming from that ecosystem.

## Usage

Add `manydns` to your project to use the abstract DNS zone management traits:

```toml
[dependencies]
manydns = { version = "0.2" }
```

### Provider Implementations

Enable provider implementations via feature flags:

| Provider                                            | Feature Flag     |
|-----------------------------------------------------|------------------|
| [Cloudflare](https://www.cloudflare.com/)           | `cloudflare`     |
| [Hetzner Cloud](https://www.hetzner.com/dns-console/) | `hetzner`      |
| [DNSPod](https://www.dnspod.cn/)                    | `dnspod`         |
| [Tencent Cloud](https://cloud.tencent.com/)         | `tencent`        |
| [Technitium](https://technitium.com/dns/)           | `technitium-dns` |
| [Namecheap](https://www.namecheap.com/)             | `namecheap`      |

Example with Cloudflare:

```toml
[dependencies]
manydns = { version = "0.2", features = ["cloudflare"] }
```

### TLS Backend

Provider implementations use [`reqwest`](https://crates.io/crates/reqwest) for HTTP communication. By default, `default-tls` is enabled. Alternative TLS backends:

- `default-tls` (default)
- `rustls-tls`
- `native-tls`
- `native-tls-vendored`

To use a different backend, disable default features:

```toml
[dependencies]
manydns = { version = "0.2", default-features = false, features = ["rustls-tls", "cloudflare"] }
```

## API Compatibility

manydns aims to provide a similar API to the Go [libdns](https://github.com/libdns/libdns) project. The core traits (`Provider`, `Zone`, `CreateRecord`, `DeleteRecord`, etc.) mirror the Go interfaces, making it easier to port code between the two ecosystems.

## Contributing

Contributions are welcome. Feel free to add new provider implementations or improve existing ones.
