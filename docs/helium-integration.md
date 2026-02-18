# Helium Network Integration

## Overview

The [Helium IoT Network](https://www.helium.com/) is a decentralized LoRaWAN network with 300,000+ hotspots globally. By purchasing an OUI (Organizationally Unique Identifier), LoraUrbit can act as a LoRaWAN Network Server (LNS) on the Helium network.

This means any LoRa device, anywhere a Helium hotspot has coverage, can route its data through LoraUrbit → Urbit instead of traditional cloud infrastructure.

## Prerequisites

### 1. Helium Wallet
- Create via [Helium Wallet App](https://docs.helium.com/wallets/helium-wallet-app)
- Fund with SOL for transaction fees
- Convert public key to legacy Helium format for OUI registration

### 2. OUI Purchase ($235 one-time)

| Item | Cost |
|------|------|
| OUI | $100 |
| DevAddr slab (8 addresses) | $100 |
| Minimum Data Credits (3.5M DC) | $35 |

Contact: hello@helium.foundation

### 3. Helium Config Service CLI (Rust)
```bash
git clone https://github.com/helium/helium-config-service-cli.git
cd helium-config-service-cli && cargo build --release
sudo cp target/release/helium-config-service-cli /usr/local/bin/
```

### 4. Key Generation
```bash
# Owner keypair (keep this VERY safe — loss = lost OUI)
helium-config-service-cli env generate-keypair owner.bin

# Delegate keypair (for day-to-day route management)
helium-config-service-cli env generate-keypair delegate.bin
```

## Route Configuration

After receiving your OUI, configure routing:

```bash
# Initialize CLI
export HELIUM_KEYPAIR_BIN=./delegate.bin
export HELIUM_NET_ID=00003C
export HELIUM_OUI=<your-oui>
export HELIUM_MAX_COPIES=15

# Create route
helium-config-service-cli route new --commit

# Set LoraUrbit endpoint
helium-config-service-cli route update server \
  --host <your-public-ip> \
  --port 1680 \
  --route-id <route-id> \
  --commit

# Add US915 region mapping
helium-config-service-cli route update add-gwmp-region \
  --route-id <route-id> \
  us915 1680 \
  --commit

# Add DevAddr range
helium-config-service-cli route devaddrs add \
  --start-addr <start> \
  --end-addr <end> \
  --route-id <route-id> \
  --commit
```

## Architecture with LoraUrbit

The key insight: Helium Packet Router can forward packets via **GWMP** (Semtech UDP) — the exact same protocol our UDP server already speaks. This means:

- **Zero code changes needed** for Phase 1 to receive Helium packets
- LoraUrbit's UDP server handles local gateways and Helium identically
- The `PacketSource` field in our types distinguishes origin for logging/analytics

## Data Credits

- 1 DC = $0.00001
- Uplink cost: 1 DC per 24-byte packet (scales with size)
- Minimum escrow: 3.5M DC ($35)
- If balance drops below 3.5M, traffic halts
- Monitor via: `helium-config-service-cli org get --oui <OUI>`

## Rust Resources from Helium

Helium's team builds in Rust — these are valuable references:

- **[gateway-rs](https://github.com/helium/gateway-rs)** — Helium Gateway daemon (tokio, Semtech GWMP, gRPC)
- **[helium-config-service-cli](https://github.com/helium/helium-config-service-cli)** — Config Service CLI
- **[helium/proto](https://github.com/helium/proto)** — Protobuf definitions for all Helium services
- **[oracles](https://github.com/helium/oracles)** — Reward and verification oracles

## DevAddr Multiplexing

Helium uses DevAddr multiplexing — multiple devices can share the same DevAddr. The LNS (LoraUrbit) disambiguates using MIC (Message Integrity Check) verification:

1. Receive uplink with DevAddr
2. Look up all session keys for that DevAddr
3. Compute MIC with each NwkSKey
4. The one that matches is the real device

This is why Phase 4 needs the `keys.rs` module for session key management.
