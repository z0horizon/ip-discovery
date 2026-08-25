# @zer0horizon/ip-discovery

Lightweight, high-performance Node.js native library to discover your public and local private IP addresses. Built on top of a robust Rust core using `napi-rs` for maximum speed and safety.

## Features

- **Fast & Lightweight**: Core implementation written in Rust with zero JavaScript runtime dependencies.
- **Multiple Protocols**: Supports **STUN** (UDP), **DNS** (UDP), and **HTTP** protocols.
- **Offline-Friendly Local IP**: Synchronous helper to query the system's primary private IP (IPv4 & IPv6) without sending network packets.
- **Multiple Resolution Strategies**:
  - `First`: Try providers in order and return the first success (default).
  - `Race`: Query all providers concurrently and return the fastest response.
  - `Consensus`: Query multiple providers and return the IP only if they agree (highly secure).
- **TypeScript Support**: Fully type-safe bindings generated out of the box.

---

## Installation

```bash
npm install @zer0horizon/ip-discovery
```

---

## Quick Start

### 1. Retrieve Public IP (Asynchronous)

```javascript
const { getIpv4, getIpv6 } = require('@zer0horizon/ip-discovery');

async function main() {
  try {
    // Get public IPv4
    const v4 = await getIpv4();
    console.log(`Public IPv4: ${v4.ip} (via ${v4.provider}, latency: ${v4.latencyMs}ms)`);

    // Get public IPv6 (fails cleanly if network doesn't support IPv6)
    const v6 = await getIpv6();
    console.log(`Public IPv6: ${v6.ip} (via ${v6.provider}, latency: ${v6.latencyMs}ms)`);
  } catch (err) {
    console.error('Lookup failed:', err.message);
  }
}

main();
```

### 2. Retrieve Local Private IP (Synchronous & Offline)

```javascript
const { getPrivateIp, getPrivateIpv6 } = require('@zer0horizon/ip-discovery');

// Returns primary network interface IP address or null if offline.
// Safe, synchronous, and doesn't send any network packets.
const localV4 = getPrivateIp();
const localV6 = getPrivateIpv6();

console.log('Local Private IPv4:', localV4 || 'Not connected');
console.log('Local Private IPv6:', localV6 || 'Not connected');
```

---

## Custom Configuration

You can use the `getIp` function with a custom configuration object to filter protocols, choose providers, or adjust the resolution strategy.

```javascript
const { getIp, IpVersion, Strategy, Protocol, BuiltinProvider } = require('@zer0horizon/ip-discovery');

async function customLookup() {
  const config = {
    timeoutMs: 5000,
    version: IpVersion.V4,
    strategy: Strategy.Race,
    protocols: [Protocol.Dns, Protocol.Stun],
    providers: [BuiltinProvider.CloudflareStun, BuiltinProvider.GoogleDns]
  };

  try {
    const result = await getIp(config);
    console.log(`Resulting IP: ${result.ip}`);
  } catch (err) {
    console.error(err.message);
  }
}
```

---

## API Reference

### Exported Functions

- **`getIp(config?: JsConfig): Promise<JsProviderResult>`**
- **`getIpv4(): Promise<JsProviderResult>`**
- **`getIpv6(): Promise<JsProviderResult>`**
- **`getPrivateIp(): string | null`**
- **`getPrivateIpv6(): string | null`**

### Type-Safe Configuration Enums

#### `IpVersion`
- `V4`
- `V6`
- `Any`

#### `Strategy`
- `First`
- `Race`
- `Consensus` (Requires consensus agreement between at least 2 providers)

#### `Protocol`
- `Dns`
- `Stun`
- `Http`

#### `BuiltinProvider`
- `CloudflareStun`
- `GoogleStun`
- `GoogleStun1`
- `GoogleStun2`
- `CloudflareDns`
- `GoogleDns`
- `OpenDns`
- `CloudflareHttp`
- `Aws`

---

## License

Dual-licensed under either:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
