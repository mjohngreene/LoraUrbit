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
├── Cargo.toml              # Rust workspace
├── README.md
├── config.toml             # Runtime configuration
├── docs/
│   ├── architecture.md     # Detailed architecture & design decisions
│   ├── helium-integration.md  # Helium OUI/LNS integration guide
│   └── semtech-udp.md      # Semtech UDP Packet Forwarder protocol reference
├── src/
│   ├── main.rs             # Entry point, async runtime
│   ├── config.rs           # Configuration loading
│   ├── udp/
│   │   ├── mod.rs          # Semtech UDP Packet Forwarder server
│   │   └── protocol.rs     # GWMP frame parsing (PUSH_DATA, PULL_DATA, etc.)
│   ├── lorawan/
│   │   ├── mod.rs          # LoRaWAN MAC layer decoder
│   │   └── keys.rs         # Session key management, MIC verification
│   ├── helium/
│   │   ├── mod.rs          # Helium network integration
│   │   └── router.rs       # Helium Packet Router client
│   └── urbit/
│       ├── mod.rs          # Urbit Airlock client
│       └── types.rs        # Poke/subscription types for %lora-agent
├── urbit/                  # Hoon code for Urbit
│   └── lora-agent/
│       ├── app/
│       │   └── lora-agent.hoon
│       ├── sur/
│       │   └── lora-agent.hoon
│       └── mar/
│           └── lora/
│               └── action.hoon
└── tests/
    ├── gateway_sim.rs      # Fake gateway simulator for testing
    └── decode_test.rs      # LoRaWAN frame decode tests
```

## Phases

### Phase 1 — Minimal Packet Receiver (current)
- Semtech UDP Packet Forwarder server (receives PUSH_DATA, sends PUSH_ACK)
- LoRaWAN PHY payload decoder (DevAddr, FCtrl, FPort, payload, MIC)
- Gateway simulator for hardware-free testing
- Configuration via TOML

### Phase 2 — Urbit Bridge
- Connect to local Urbit ship via Airlock HTTP API
- Forward decoded packets as pokes to `%lora-agent`
- Subscription support for real-time data streaming

### Phase 3 — Gall Agent
- `%lora-agent`: receives pokes, stores device state, publishes on paths
- Remote ships subscribe over Ames for sensor data
- No HTTP involved — pure Ames transport

### Phase 4 — Helium Integration
- OUI registration and route configuration
- Helium Packet Router client (gRPC)
- DevAddr management and session key handling
- Data Credit monitoring

### Phase 5 — Full Ames Routing
- Bidirectional: sensor data up, commands down
- Device management over Ames
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
