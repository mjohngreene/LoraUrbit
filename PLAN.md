# LoraUrbit — Implementation Plan (Hoon-First)

> Updated 2026-02-19. Prioritizes Hoon for all application logic.
> Rust is a thin translation layer for hardware protocols only.

## Design Principle

**Hoon owns the brains. Rust owns the wire.**

Everything that involves application logic — device state, subscriptions, access
control, data routing, multi-ship peering — lives in Hoon as a Gall agent.

Rust handles only what Urbit *can't*: raw UDP sockets, binary protocol parsing,
and Helium's gRPC interface. The Rust bridge is intentionally dumb — it decodes
packets and hands them to the ship, nothing more.

---

## Phase 1 — Minimal Packet Receiver ✅ DONE

**Language:** Rust
**Goal:** Receive and decode LoRaWAN packets without hardware.

- [x] Semtech UDP server (GWMP: PUSH_DATA, PUSH_ACK, PULL_DATA, PULL_ACK)
- [x] LoRaWAN PHY payload decoder (MHDR, DevAddr, FCtrl, FCnt, FPort, MIC)
- [x] Gateway simulator for hardware-free testing
- [x] Configuration via TOML
- [x] All tests passing

**Deliverable:** `cargo run` receives simulated LoRa packets and decodes them.

---

## Phase 2 — Gall Agent + Airlock Bridge (CURRENT)

**Language:** Hoon (agent) + Rust (Airlock client)
**Goal:** Decoded packets flow into Urbit and persist on-ship.

### 2a — Hoon: `%lora-agent` Gall App

- [ ] **Sur file** (`sur/lora-agent.hoon`): Core types
  - `uplink`: decoded packet (dev-addr, fcnt, f-port, payload, rssi, snr, freq, data-rate, gateway-eui, received-at, mtype, source)
  - `device`: registered device (dev-addr, name, description, last-seen, packet-count)
  - `action`: poke types (uplink, register-device, downlink-request)
  - `update`: subscription update types
- [ ] **Mar files** (`mar/lora/*.hoon`): JSON ↔ Hoon marks
  - `mar/lora/action.hoon`: JSON parsing for pokes from Rust bridge
  - `mar/lora/update.hoon`: JSON serialization for subscription updates
- [ ] **App file** (`app/lora-agent.hoon`): Gall agent
  - State: `(map @t device)` device registry, `(list uplink)` recent packets
  - `on-poke`: handle uplink, register-device, downlink-request
  - `on-watch`: subscription paths `/devices`, `/uplinks`, `/uplinks/<dev-addr>`
  - `on-leave`: cleanup subscriptions
  - `on-agent`: handle remote ship interactions (Phase 3)
  - `on-peek`: scry endpoints for device state, packet history

### 2b — Rust: Lightweight Airlock Client

- [ ] `src/urbit/airlock.rs`: HTTP client (~150-200 lines)
  - `login()`: POST `/~/login` with +code, store cookie
  - `poke()`: PUT `/~/channel/<uid>` with action JSON
  - Message ID tracking
- [ ] Wire into main loop: UDP packet decoded → poke `%lora-agent` via Airlock
- [ ] Reconnect/retry logic

### 2c — Integration

- [ ] Install `%lora-agent` on test ship (fake ship or `~fipmus-modsyr-tapper-botsub`)
- [ ] Run gateway simulator → Rust decodes → pokes agent → agent stores
- [ ] Verify state via dojo scry: `.^(* %gx /=lora-agent=/devices/noun)`

**Deliverable:** End-to-end flow from simulated LoRa packet to persistent Urbit state.

---

## Phase 3 — Ames Subscriptions & Downlinks

**Language:** Hoon (all new work)
**Goal:** Other ships subscribe to sensor data over Ames. Downlinks work.

- [ ] Remote subscription: ship B pokes ship A's `%lora-agent` to subscribe
- [ ] Agent publishes updates on watched paths when new uplinks arrive
- [ ] Downlink queue: poke agent with downlink request → agent stores in queue
- [ ] Rust bridge scries `/=lora-agent=/downlink-queue/json` periodically
- [ ] Bridge sends queued downlinks as PULL_RESP via UDP to gateway
- [ ] Agent confirms downlink sent (bridge pokes back with TX_ACK result)

**Deliverable:** Ship-to-ship sensor data streaming. Commands flow back to devices.

---

## Phase 4 — Helium OUI Integration

**Language:** Rust (gRPC client) + minimal Hoon (DC monitoring)
**Goal:** Receive packets from any Helium hotspot worldwide.

- [ ] OUI purchase ($235) and route configuration
- [ ] Helium Packet Router speaks GWMP — zero changes to UDP server
- [ ] Rust gRPC client for Config Service (route management, DevAddr slabs)
- [ ] MIC verification + AES payload decryption in Rust (`keys.rs`)
- [ ] Hoon: Data Credit balance display, device EUI management

**Deliverable:** Any LoRa device within Helium coverage routes through LoraUrbit.

---

## Phase 5 — Polish & Distribution

**Language:** Hoon
**Goal:** Installable desk, web UI, multi-ship topologies.

- [ ] Package as distributable Urbit desk (`%lora`)
- [ ] Web UI via Landscape tile or standalone (device dashboard, packet log)
- [ ] Device management UX (naming, grouping, alerting)
- [ ] Multi-ship mesh: ships relay data across the network
- [ ] Documentation for end users

---

## What Lives Where

| Component | Language | Why |
|-----------|----------|-----|
| UDP server (Semtech GWMP) | Rust | Raw UDP sockets, binary protocol |
| LoRaWAN PHY decoder | Rust | Binary parsing, performance |
| Airlock HTTP client | Rust | Thin bridge, ~150 lines |
| Gateway simulator | Rust | Testing tool |
| MIC verification / AES | Rust | Crypto primitives |
| Helium gRPC client | Rust | Protobuf/gRPC |
| **Device registry** | **Hoon** | Persistent state, native to Urbit |
| **Packet storage** | **Hoon** | Event log, no external DB needed |
| **Subscription routing** | **Hoon** | Ames subscriptions, native to Urbit |
| **Access control** | **Hoon** | Ship identity, native to Urbit |
| **Downlink queue** | **Hoon** | Agent state, poke-driven |
| **Multi-ship peering** | **Hoon** | Ames, native to Urbit |
| **Web UI** | **Hoon** | Sail/Landscape, on-ship |

---

## File Layout

```
LoraUrbit/
├── Cargo.toml                    # Rust: bridge binary
├── config.toml                   # Rust: bridge configuration
├── src/                          # Rust: thin bridge
│   ├── main.rs
│   ├── config.rs
│   ├── udp/                      # Semtech GWMP server
│   ├── lorawan/                  # LoRaWAN decoder + crypto
│   ├── helium/                   # Helium gRPC (Phase 4)
│   └── urbit/
│       ├── airlock.rs            # Lightweight Airlock client
│       └── types.rs              # Shared types (Rust ↔ JSON ↔ Hoon)
├── urbit/                        # Hoon: the brains
│   └── lora/                     # Desk name: %lora
│       ├── app/
│       │   └── lora-agent.hoon   # Gall agent
│       ├── sur/
│       │   └── lora-agent.hoon   # Type definitions
│       ├── mar/
│       │   └── lora/
│       │       ├── action.hoon   # Poke mark (JSON → Hoon)
│       │       └── update.hoon   # Subscription mark (Hoon → JSON)
│       ├── lib/
│       │   └── lora.hoon         # Helper library (optional)
│       └── desk.bill             # Agent manifest
├── docs/
├── tests/
└── README.md
```
