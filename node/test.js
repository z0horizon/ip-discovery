const test = require('node:test');
const assert = require('node:assert');
const net = require('node:net');
const { getIp, getIpv4, getIpv6, getPrivateIp, getPrivateIpv6, IpVersion, Strategy, Protocol, BuiltinProvider } = require('./index.js');

// Public-network tests are opt-in so the default suite is deterministic/offline-safe.
function testNetwork(name, fn) {
  test(name, { skip: process.env.IP_DISCOVERY_NETWORK_TESTS !== '1' }, fn);
}

testNetwork('getIpv4 should retrieve a valid IPv4 address', async () => {
  const result = await getIpv4();
  assert.ok(result.ip);
  assert.ok(result.provider);
  assert.ok(result.protocol);
  assert.ok(result.latencyMs >= 0);
  assert.ok(net.isIPv4(result.ip), 'IP should be a valid IPv4 address');
});

testNetwork('getIpv6 should retrieve a valid IPv6 address', async () => {
  const result = await getIpv6();
  assert.ok(result.ip);
  assert.ok(result.provider);
  assert.ok(result.protocol);
  assert.ok(net.isIPv6(result.ip), 'IP should be a valid IPv6 address');
});

testNetwork('getIp with no config should resolve correctly', async () => {
  const result = await getIp();
  assert.ok(result.ip);
  assert.ok(result.provider);
  assert.ok(result.protocol);
});

// Test all strategies using enums
testNetwork('getIp with strategy: first should work', async () => {
  const result = await getIp({ strategy: Strategy.First });
  assert.ok(result.ip);
});

testNetwork('getIp with strategy: race should work', async () => {
  const result = await getIp({ strategy: Strategy.Race });
  assert.ok(result.ip);
});

testNetwork('getIp with strategy: consensus should work', async () => {
  const result = await getIp({ strategy: Strategy.Consensus });
  assert.ok(result.ip);
});

// Test all IP versions configurations
testNetwork('getIp with version: v4 should return IPv4', async () => {
  const result = await getIp({ version: IpVersion.V4 });
  assert.ok(net.isIPv4(result.ip), 'IP should be a valid IPv4 address');
});

testNetwork('getIp with version: any should work', async () => {
  const result = await getIp({ version: IpVersion.Any });
  assert.ok(result.ip);
});

// Test all protocols individually and combined
testNetwork('getIp with protocol: dns should work and return DNS protocol', async () => {
  const result = await getIp({ protocols: [Protocol.Dns] });
  assert.ok(result.ip);
  assert.strictEqual(result.protocol, 'DNS');
});

testNetwork('getIp with protocol: stun should work and return STUN protocol', async () => {
  const result = await getIp({ protocols: [Protocol.Stun] });
  assert.ok(result.ip);
  assert.strictEqual(result.protocol, 'STUN');
});

testNetwork('getIp with protocol: http should work and return HTTP protocol', async () => {
  const result = await getIp({ protocols: [Protocol.Http] });
  assert.ok(result.ip);
  assert.strictEqual(result.protocol, 'HTTP');
});

testNetwork('getIp with protocols: dns + stun combined should work', async () => {
  const result = await getIp({ protocols: [Protocol.Dns, Protocol.Stun] });
  assert.ok(result.ip);
  assert.ok(['DNS', 'STUN'].includes(result.protocol));
});

// Test every single built-in provider to check enums are mapped correctly
const providersToTest = [
  { value: BuiltinProvider.CloudflareStun, name: 'CloudflareStun', expectedProtocol: 'STUN', expectedProvider: 'Cloudflare STUN' },
  { value: BuiltinProvider.GoogleStun, name: 'GoogleStun', expectedProtocol: 'STUN', expectedProvider: 'Google STUN' },
  { value: BuiltinProvider.GoogleStun1, name: 'GoogleStun1', expectedProtocol: 'STUN', expectedProvider: 'Google STUN 1' },
  { value: BuiltinProvider.GoogleStun2, name: 'GoogleStun2', expectedProtocol: 'STUN', expectedProvider: 'Google STUN 2' },
  { value: BuiltinProvider.CloudflareDns, name: 'CloudflareDns', expectedProtocol: 'DNS', expectedProvider: 'Cloudflare DNS' },
  { value: BuiltinProvider.GoogleDns, name: 'GoogleDns', expectedProtocol: 'DNS', expectedProvider: 'Google DNS' },
  { value: BuiltinProvider.OpenDns, name: 'OpenDns', expectedProtocol: 'DNS', expectedProvider: 'OpenDNS' },
  { value: BuiltinProvider.CloudflareHttp, name: 'CloudflareHttp', expectedProtocol: 'HTTP', expectedProvider: 'Cloudflare' },
  { value: BuiltinProvider.Aws, name: 'Aws', expectedProtocol: 'HTTP', expectedProvider: 'AWS' },
];

for (const provider of providersToTest) {
  testNetwork(`getIp with provider enum: ${provider.name} should resolve using correct protocol and provider name`, async () => {
    const config = {
      providers: [provider.value],
      version: IpVersion.V4
    };
    const result = await getIp(config);
    assert.ok(result.ip);
    assert.strictEqual(result.protocol, provider.expectedProtocol);
    assert.strictEqual(result.provider, provider.expectedProvider);
  });
}

testNetwork('getIp with multiple providers combined should work', async () => {
  const result = await getIp({
    providers: [BuiltinProvider.CloudflareDns, BuiltinProvider.GoogleDns],
    version: IpVersion.V4
  });
  assert.ok(result.ip);
  assert.strictEqual(result.protocol, 'DNS');
  assert.ok(['Cloudflare DNS', 'Google DNS'].includes(result.provider));
});

// Combined advanced configuration test
testNetwork('getIp with complex advanced configuration should work', async () => {
  const result = await getIp({
    version: IpVersion.V4,
    strategy: Strategy.Race,
    protocols: [Protocol.Dns, Protocol.Stun],
    timeoutMs: 4000
  });
  assert.ok(result.ip);
  assert.ok(net.isIPv4(result.ip), 'IP should be a valid IPv4 address');
});

// Error handling validations for invalid types
test('getIp with invalid options should reject with type error', async () => {
  await assert.rejects(
    async () => {
      await getIp({ version: 99 });
    },
    /does not match|Failed to convert/
  );

  await assert.rejects(
    async () => {
      await getIp({ strategy: 'invalid' });
    },
    /does not match|Failed to convert/
  );

  await assert.rejects(
    async () => {
      await getIp({ protocols: ['invalid'] });
    },
    /does not match|Failed to convert/
  );

  await assert.rejects(
    async () => {
      await getIp({ providers: ['invalid'] });
    },
    /does not match|Failed to convert/
  );
});

test('getIp should reject with a timeout error if timeoutMs is too short', async () => {
  await assert.rejects(
    async () => {
      await getIp({ timeoutMs: 0, strategy: Strategy.First });
    },
    /IP discovery failed/
  );
});

testNetwork('getIp should handle multiple concurrent requests', async () => {
  const results = await Promise.all([
    getIpv4(),
    getIp({ protocols: [Protocol.Dns] }),
    getIp({ providers: [BuiltinProvider.CloudflareStun] }),
    getIp({ strategy: Strategy.Race })
  ]);
  assert.strictEqual(results.length, 4);
  for (const result of results) {
    assert.ok(result.ip);
    assert.ok(result.provider);
    assert.ok(result.protocol);
    assert.ok(result.latencyMs >= 0);
  }
});

test('getPrivateIp should retrieve a valid private IPv4 address or return null/undefined', () => {
  const ip = getPrivateIp();
  if (ip) {
    assert.ok(net.isIPv4(ip), 'Private IP should be a valid IPv4 address');
  } else {
    assert.strictEqual(ip, null);
  }
});

test('getPrivateIpv6 should retrieve a valid private IPv6 address or return null/undefined', () => {
  const ip = getPrivateIpv6();
  if (ip) {
    assert.ok(net.isIPv6(ip), 'Private IPv6 should be a valid IPv6 address');
  } else {
    assert.strictEqual(ip, null);
  }
});
