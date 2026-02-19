# LoraUrbit

**Sovereign LoRaWAN infrastructure powered by Urbit's Ames protocol.**

LoraUrbit replaces the traditional HTTP/MQTT application transport in LoRaWAN with Urbit's Ames networking protocol — providing cryptographically authenticated, end-to-end encrypted, identity-native IoT data routing.

## Vision

The standard LoRaWAN stack routes packets like this:

```
End Device → (RF) → Gateway → (IP/UDP) → Network Server → (HTTP/MQTT) → Application Server
```

LoraUrbit replaces everything after the gateway with sovereign infrastructure:

```
End Device → (RF) → Gateway → (Semtech UDP) → LoraUrbit Bridge → (Airlock) → Urbit Ship → (Ames) → Subscribers
```

Additionally, LoraUrbit integrates with the **Helium IoT Network** — a decentralized public LoRaWAN network. By purchasing an OUI (Organizationally Unique Identifier), LoraUrbit can receive packets from any Helium hotspot worldwide and route them through Urbit's Ames protocol instead of traditional cloud endpoints.

## Why?

- **Sovereign infrastructure** — no dependency on AWS/GCP for your IoT data
- **Identity built in** — every Urbit ship has cryptographic identity; devices inherit this
- **Encrypted by default** — Ames is E2E encrypted between ships
- **Decentralized routing** — no single point of failure
- **Helium integration** — tap into 300k+ hotspots globally via OUI
- **Censorship resistant** — Ames packets traverse NATs via galaxy routing

## Architecture

```
                                                    ┌──────────────────────┐
                                                    │   Remote Urbit       │
                                                    │   Ships              │
                                                    │   (Ames subscribers) │
                                                    └──────────▲───────────┘
                                                               │
                                                          Ames protocol
                                                               │
┌─────────────┐    RF     ┌──────────┐   Semtech   ┌──────────┴───────────┐
│  LoRa End   │ ────────► │  LoRa    │   UDP/GWMP  │   LoraUrbit          │
│  Device     │           │  Gateway │ ──────────► │   Bridge (Rust)      │
└─────────────┘           └──────────┘             │                      │
                                                    │   ┌─ UDP Server     │
┌─────────────┐                                     │   ├─ LoRaWAN Decode │
│  Helium     │  Helium Packet Router               │   ├─ Helium Client  │
│  Network    │ ──────────────────────────────────► │   └─ Urbit Airlock  │
│  (OUI)      │                                     │                      │
└─────────────┘                                     └──────────┬───────────┘
                                                               │
                                                        Airlock HTTP API
                                                               │
                                                    ┌──────────▼───────────┐
                                                    │   Urbit Ship         │
                                                    │   %lora-agent        │
                                                    │   (Gall app)         │
                                                    └──────────────────────┘
```

## Project Structure

```
LoraUrbit/
├── Cargo.toml                    # Rust: bridge binary
├── config.toml                   # Rust: bridge runtime config
├── src/                          # Rust: thin translation layer
│   ├── main.rs                   # Entry point, async runtime
│   ├── config.rs                 # Configuration loading
│   ├── udp/                      # Semtech GWMP server
│   │   ├── mod.rs
│   │   └── protocol.rs
│   ├── lorawan/                  # LoRaWAN decoder + crypto
│   │   ├── mod.rs
│   │   └── keys.rs
│   ├── helium/                   # Helium gRPC (Phase 4)
│   │   ├── mod.rs
│   │   └── router.rs
│   └── urbit/                    # Airlock client
│       ├── airlock.rs            # Lightweight HTTP client
│       └── types.rs              # Shared types (Rust ↔ JSON ↔ Hoon)
├── urbit/                        # Hoon: the brains (desk: %lora)
│   └── lora/
│       ├── app/
│       │   └── lora-agent.hoon   # Gall agent
│       ├── sur/
│       │   └── lora-agent.hoon   # Type definitions
│       ├── mar/
│       │   └── lora/
│       │       ├── action.hoon   # Poke mark (JSON → noun)
│       │       └── update.hoon   # Subscription mark (noun → JSON)
│       ├── lib/
│       │   └── lora.hoon         # Helper library
│       └── desk.bill             # Agent manifest
├── docs/
│   ├── architecture.md
│   ├── helium-integration.md
│   └── semtech-udp.md
└── tests/
    └── gateway_sim.rs            # Fake gateway simulator
```

## Design Principle

**Hoon owns the brains. Rust owns the wire.**

All application logic — device state, subscriptions, access control, multi-ship
peering — lives in Hoon as a Gall agent. Rust is a thin translation layer that
handles raw UDP sockets and binary protocol parsing, then hands structured JSON
to the ship via Airlock. See [PLAN.md](PLAN.md) for details.

## Phases

### Phase 1 — Minimal Packet Receiver ✅
- Semtech UDP server (receives PUSH_DATA, sends PUSH_ACK)
- LoRaWAN PHY payload decoder (DevAddr, FCtrl, FPort, payload, MIC)
- Gateway simulator for hardware-free testing
- Configuration via TOML

### Phase 2 — Gall Agent + Airlock Bridge (current)
- **Hoon:** `%lora-agent` Gall app — device state, poke handlers, scry endpoints, subscription paths
- **Rust:** Lightweight Airlock client (~150 LOC) — login, poke, done
- End-to-end: simulated packet → Rust decode → Airlock poke → Hoon agent stores

### Phase 3 — Ames Subscriptions & Downlinks
- Remote ships subscribe to sensor data over Ames (pure Hoon)
- Downlink queue in agent state, bridge polls via scry
- Ship-to-ship encrypted data streaming — no cloud involved

### Phase 4 — Helium OUI Integration
- OUI purchase ($235), route configuration
- Helium packets arrive via same GWMP protocol — zero code changes to UDP server
- Rust gRPC client for Config Service, MIC verification, AES decryption

### Phase 5 — Polish & Distribution
- Distributable Urbit desk (`%lora`)
- Web UI for device dashboard
- Multi-ship mesh topologies

## Development

```bash
# Build
cargo build

# Run with gateway simulator
cargo run

# Run tests
cargo test
```

## Requirements

- Rust 1.75+
- An Urbit ship (for Phase 2+)
- A Helium OUI (for Phase 4+, ~$235 one-time)
- LoRa gateway hardware (optional — simulator included)

## License

MIT
