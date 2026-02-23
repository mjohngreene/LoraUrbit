# LoraUrbit — Implementation Plan (Revised)

> Updated 2026-02-23. Corrects the architecture to reflect true peer-to-peer
> LoRaWAN communication. Both ships are Class C endpoints on the network.
> The gateway is neutral infrastructure that routes packets between them.

## Design Principles

**Hoon owns the brains. Rust owns the wire.**

**Ships are peers, not hub-and-spoke.** Each ship has its own full stack:
gateway + bridge + agent. No ship is "the server." Both send and receive.

**The LoRa radio layer IS the transport.** Ames is for identity, discovery,
and fallback — not for tunneling sensor data. Data flows over RF via the
gateway network.

---

## Real-World Topology

```
     Site A (e.g. Houston)                  Site B (e.g. Austin)
┌─────────────────────────┐          ┌─────────────────────────┐
│  ~zod (%lora-agent)     │          │  ~bus (%lora-agent)     │
│      ↕ Airlock          │          │      ↕ Airlock          │
│  Bridge A               │          │  Bridge B               │
│      ↕ Semtech UDP      │          │      ↕ Semtech UDP      │
│  Gateway A              │          │  Gateway B              │
└─────────┬───────────────┘          └───────────┬─────────────┘
          │          Helium Network               │
          └──────────────┬────────────────────────┘
                   (or local link for testing)
```

Each "site" is a self-contained stack. You can move an entire site to another
machine by changing IPs. The gateway-to-gateway link is the ONLY thing that
changes between local testing → LAN → Helium production.

## Testing Topology (single Mac mini)

```
~zod (%lora-agent)                          ~bus (%lora-agent)
    ↕ Airlock (localhost:8080)                  ↕ Airlock (localhost:8081)
Bridge A (UDP 1680)                         Bridge B (UDP 1681)
    ↕ Semtech UDP                               ↕ Semtech UDP
Gateway A (UDP 1700)                        Gateway B (UDP 1701)
    └──────── localhost UDP link ───────────────┘
```

All processes on 127.0.0.1. Migration path:
1. **Local**: All on one machine (current)
2. **LAN**: Move Site B to another box on 192.168.1.x
3. **Helium**: Replace gateway-to-gateway link with Helium packet router

---

## DevAddr Model

### How DevAddrs Work

LoraUrbit operates a single Helium OUI, which owns a slab of 8 DevAddrs
(expandable). DevAddrs are assigned to ships by the LNS during the LoRaWAN
join procedure (OTAA — Over The Air Activation):

1. New ship sends a Join Request through its local gateway
2. Join Request reaches the LNS (via Helium or local link)
3. LNS assigns a DevAddr from the pool → sends Join Accept back
4. That DevAddr is **permanently** associated with the ship's Urbit identity
5. All future traffic from that DevAddr is attributed to that ship

Per Helium docs, 8 DevAddrs can support 100k+ devices via multiplexing
(MIC check disambiguates). More DevAddrs can be purchased at any time.

### Phase 3 (testing): Static assignment
- DevAddrs hardcoded in bridge config files
- Agents use `%set-identity` (my addr) and `%add-peer` (their addr)
- No join procedure — manual setup for two-ship testing

### Phase 4 (production): Dynamic join via LNS
- LNS implements OTAA Join Server
- Ships join the network automatically
- DevAddr assignment is centralized on the LNS
- Mapping shared with per-ship agents after join

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

## Phase 2 — Gall Agent + Airlock Bridge ✅ DONE

**Language:** Hoon (agent) + Rust (Airlock client)
**Goal:** Decoded packets flow into Urbit and persist on-ship.

- [x] `%lora-agent` Gall app — device state, poke handlers, scry endpoints
- [x] Lightweight Airlock client (~200 LOC) — login, poke, reconnect
- [x] End-to-end: simulated packet → Rust decode → Airlock poke → agent stores
- [x] Subscription paths `/uplinks`, `/devices` with JSON updates
- [x] Scry endpoints `/stats`, `/devices`

**Deliverable:** Simulated LoRa packet reaches Urbit ship and persists.

---

## Phase 3 — Peer-to-Peer LoRa Communication (CURRENT)

**Language:** Rust (bridge + gateway sim) + Hoon (agent rework)
**Goal:** Two ships communicate bidirectionally through LoRa gateways.

### What changed from the old plan

The old Phase 3 had Ship B poking Ship A's agent to subscribe over Ames.
That's wrong — it reimplements a cloud server on Urbit. The correct model:
both ships are Class C LoRaWAN endpoints. Each has its own gateway. Data
flows over the LoRa radio layer, not over Ames.

### 3a — Bidirectional Bridge (Rust)

Currently the bridge is receive-only (uplinks in). It needs to also transmit
(downlinks out) so a ship can send data TO the LoRa network.

- [ ] **TX support**: Send PULL_RESP to gateway for downlink transmission
  - Track gateway address from PULL_DATA keepalives
  - Build PULL_RESP with TX parameters (freq, power, data rate, timing)
  - Handle TX_ACK confirmation from gateway
- [ ] **Outbound message queue**: Scry `%lora-agent` for pending outbound messages
  - Poll `/=lora-agent=/outbox/json` on interval (e.g. every 2 seconds)
  - Convert Hoon message → LoRaWAN frame → base64 payload → txpk JSON
  - Poke agent with `%tx-ack` after gateway confirms
- [ ] **Per-site config**: Support running as "Site A" or "Site B"
  - Each bridge has its own UDP port, Urbit ship URL, and +code
  - Config: `config-a.toml`, `config-b.toml`

### 3b — Gateway Pair Simulator (Rust, new binary)

Two simulated gateways linked by a localhost UDP pipe. Replaces the old
single `gateway_sim.rs` which was one-way only.

- [ ] **Gateway A** (UDP 1700): Accepts PUSH_DATA from Bridge A,
  forwards as PUSH_DATA to Gateway B → Bridge B
- [ ] **Gateway B** (UDP 1701): Accepts PUSH_DATA from Bridge B,
  forwards as PUSH_DATA to Gateway A → Bridge A
- [ ] **PULL_DATA handling**: Both gateways send keepalives to their bridge,
  and accept PULL_RESP (downlinks) back
- [ ] **The inter-gateway link**: Gateway A ↔ Gateway B is a simple UDP
  relay on localhost. This is what gets replaced by Helium later.
- [ ] **Packet translation**: When Gateway A receives an uplink from Bridge A,
  it re-wraps it as a PUSH_DATA and sends to Gateway B (which forwards to
  Bridge B). This simulates the Helium routing path.

Binary: `cargo run --bin gateway-pair`

### 3c — Peer-to-Peer Agent (Hoon, rework)

Rework `%lora-agent` from a hub-and-spoke data collector to a peer-to-peer
messaging agent. Each ship runs an identical agent.

- [ ] **Remove**: `%subscribe-remote` poke, `on-agent` remote subscription handler
- [ ] **New state**:
  - `peers`: `(map @p [dev-addr=@t last-seen=@da status=?(%online %offline)])`
    — maps Urbit ship identity to LoRa device address
  - `outbox`: `(list outbound-msg)` — messages waiting for bridge to transmit
  - `inbox`: `(list inbound-msg)` — received messages with sender identity
- [ ] **New types** (in `sur/lora-agent.hoon`):
  - `outbound-msg`: `[id=@ud dest-ship=@p dest-addr=@t payload=@t queued-at=@da sent=?]`
  - `inbound-msg`: `[id=@ud src-ship=(unit @p) src-addr=@t payload=@t received-at=@da]`
  - `peer`: `[=ship dev-addr=@t last-seen=@da status=?(%online %offline)]`
- [ ] **New poke actions**:
  - `%register-peer`: associate a DevAddr with a ship identity
    `{action: "register-peer", ship: "~bus", dev-addr: "01AB5678"}`
  - `%send-message`: queue a message for a peer
    `{action: "send-message", dest: "~bus", payload: "48656C6C6F"}`
  - `%message-received`: bridge pokes when inbound LoRa message arrives
    (replaces `%uplink` — now includes peer resolution)
  - `%tx-ack`: bridge confirms message was transmitted
  - `%tx-fail`: bridge reports transmission failure
- [ ] **New scry endpoints**:
  - `/outbox/json` — pending outbound messages (bridge polls this)
  - `/inbox/json` — received messages
  - `/peers/json` — known peers and their status
- [ ] **Peer resolution on receive**: When a LoRa packet arrives with a DevAddr,
  the agent looks up the peer map to identify the sender ship. If unknown,
  stores with `src-ship=~`.
- [ ] **Keep existing uplink/device functionality**: Raw packet tracking is still
  useful for diagnostics. The peer messaging layer sits on top.

### 3d — End-to-End Integration Test

- [ ] Start two fake ships: ~zod (port 8080) and ~bus (port 8081)
- [ ] Start gateway pair: Gateway A (1700) ↔ Gateway B (1701)
- [ ] Start two bridges: Bridge A (1680 → ~zod) and Bridge B (1681 → ~bus)
- [ ] On ~zod: `:lora-agent &json '{"action":"register-peer","ship":"~bus","dev-addr":"01AB5678"}'`
- [ ] On ~bus: `:lora-agent &json '{"action":"register-peer","ship":"~zod","dev-addr":"260B1234"}'`
- [ ] On ~zod: `:lora-agent &json '{"action":"send-message","dest":"~bus","payload":"48656C6C6F"}'`
- [ ] Verify: Bridge A polls outbox → sends PULL_RESP to Gateway A →
  Gateway A forwards to Gateway B → Bridge B receives PUSH_DATA →
  Bridge B pokes ~bus with `%message-received` → ~bus identifies sender as ~zod
- [ ] Test reverse direction: ~bus → ~zod
- [ ] Verify scry endpoints on both ships show correct state

**Deliverable:** Two Urbit ships exchange messages over simulated LoRa gateways.
Bidirectional. Identity-aware. No Ames dependency for data transport.

---

## Phase 4 — Helium OUI Integration + LNS Service

**Language:** Rust (gRPC client + LNS logic) + Hoon (LNS agent)
**Goal:** Replace localhost gateway link with Helium's global network.
Operate a single OUI as a service for all LoraUrbit users.

### Business Model

LoraUrbit operates a **single Helium OUI** as shared infrastructure.
Any Urbit ship owner can join the network, get a DevAddr assigned, and
communicate over Helium. Users pay a fee to cover Data Credit consumption.

You are building a **sovereign LoRaWAN service provider** — an ISP for
Urbit-native IoT. Users bring their own ship + gateway hardware. You
provide the network routing layer. Their data flows peer-to-peer over
LoRa — the LNS routes packets but doesn't read them.

### Architecture

```
┌─────────────────────────────────────────────────────────┐
│  LoraUrbit LNS (your infrastructure)                    │
│                                                         │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────────┐ │
│  │ OUI Manager  │  │ DevAddr Pool │  │ Billing /     │ │
│  │ (gRPC ↔      │  │ (8+ addrs,   │  │ DC Tracking   │ │
│  │  Helium      │  │  assign at   │  │ (per-user     │ │
│  │  Config Svc) │  │  join)       │  │  metering)    │ │
│  └──────────────┘  └──────────────┘  └───────────────┘ │
│                                                         │
│  ┌──────────────────────────────────────────────────┐   │
│  │ Join Server                                      │   │
│  │ - Receives Join Requests from new ships          │   │
│  │ - Assigns DevAddr from pool                      │   │
│  │ - Records DevAddr ↔ ship identity permanently    │   │
│  │ - Sends Join Accept back through gateway          │   │
│  └──────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
         │
         │  Helium Packet Router (GWMP / gRPC)
         │
    ┌────┴────────────────────────────────┐
    │         Helium Network              │
    │         (300k+ hotspots)            │
    └────┬───────────┬───────────┬────────┘
         │           │           │
      User A      User B      User C
      (~zod)      (~bus)      (~sampel)
      Gateway     Gateway     Gateway
      Bridge      Bridge      Bridge
      Ship        Ship        Ship
```

### Implementation

- [ ] OUI purchase ($235) and route configuration
- [ ] LNS service (Rust or separate Gall agent `%lora-lns`):
  - DevAddr pool management (assign from OUI's slab of 8)
  - OTAA Join Server: receive Join Request → assign DevAddr → Join Accept
  - DevAddr ↔ ship identity registry (permanent after join)
  - Helium Config Service gRPC client (route management)
  - Per-user DC consumption tracking for billing
- [ ] MIC verification + AES payload decryption in Rust (`keys.rs`)
- [ ] Hoon per-ship agent: replace static DevAddr with join-assigned addr
- [ ] Data Credit balance monitoring and low-balance alerts

**Pre-Helium milestone:** Move Site B (Gateway B + Bridge B + ~bus) to another
machine on LAN. Verify everything works with real network hops. Then swap
the LAN link for Helium.

**Deliverable:** Multi-user LoRaWAN service. Any Urbit ship joins the OUI,
gets a DevAddr, and communicates with any other ship on the network.

---

## Phase 5 — Polish & Distribution

**Language:** Hoon + Rust
**Goal:** Installable desk, web UI, real hardware, multi-user scaling.

- [ ] Package `%lora-agent` as distributable Urbit desk (`%lora`)
- [ ] Web UI via Landscape tile (peer status, message log, send interface)
- [ ] Real LoRa gateway hardware integration (RAK, Dragino, etc.)
- [ ] Multi-peer mesh: >2 ships, group messaging, topic channels
- [ ] Class C device management (RX2 windows, beacon timing)
- [ ] End-to-end encryption at the LoRaWAN layer (AppSKey per peer pair)
- [ ] User onboarding flow (join network, pay fee, get DevAddr)
- [ ] LNS dashboard (user count, DC balance, traffic metrics)
- [ ] DevAddr pool expansion (buy more slabs as user base grows)
- [ ] Documentation for end users and LNS operators

---

## What Lives Where

| Component | Language | Why |
|-----------|----------|-----|
| UDP server (Semtech GWMP) | Rust | Raw UDP sockets, binary protocol |
| LoRaWAN PHY decoder/encoder | Rust | Binary parsing, performance |
| Airlock HTTP client | Rust | Thin bridge, ~200 lines |
| Gateway pair simulator | Rust | Testing tool, localhost relay |
| MIC verification / AES | Rust | Crypto primitives |
| Helium gRPC client | Rust | Protobuf/gRPC |
| **Peer registry** | **Hoon** | Ship identity ↔ DevAddr mapping |
| **Message inbox/outbox** | **Hoon** | Persistent state, event log |
| **Device tracking** | **Hoon** | Diagnostics, uplink history |
| **Access control** | **Hoon** | Ship identity, native to Urbit |
| **Web UI** | **Hoon** | Sail/Landscape, on-ship |

---

## File Layout (Revised)

```
LoraUrbit/
├── Cargo.toml
├── config-a.toml                 # Site A config (Bridge A → ~zod)
├── config-b.toml                 # Site B config (Bridge B → ~bus)
├── config.toml                   # Legacy / default config
├── src/
│   ├── main.rs                   # Bridge binary entry point
│   ├── config.rs
│   ├── udp/
│   │   ├── mod.rs                # UDP server (rx + tx)
│   │   └── protocol.rs          # GWMP parsing + building
│   ├── lorawan/
│   │   ├── mod.rs                # LoRaWAN decoder
│   │   ├── encoder.rs            # LoRaWAN frame builder (NEW)
│   │   └── keys.rs               # Session keys (Phase 4)
│   ├── helium/
│   │   ├── mod.rs
│   │   └── router.rs
│   └── urbit/
│       ├── mod.rs
│       ├── airlock.rs            # HTTP client (poke + scry)
│       └── types.rs              # Shared types
├── src/bin/
│   ├── gateway_sim.rs            # Legacy single-gateway sim (keep)
│   └── gateway_pair.rs           # NEW: paired gateway simulator
├── urbit/lora/                   # Hoon desk: %lora
│   ├── app/
│   │   └── lora-agent.hoon      # Gall agent (peer-to-peer)
│   ├── sur/
│   │   └── lora-agent.hoon      # Type definitions (revised)
│   ├── mar/lora/
│   │   ├── action.hoon           # Poke mark
│   │   └── update.hoon           # Subscription mark
│   ├── lib/
│   │   └── lora.hoon             # Helper library
│   └── desk.bill
├── docs/
│   ├── architecture.md           # Updated for peer-to-peer
│   ├── helium-integration.md
│   └── semtech-udp.md
└── README.md
```

---

## Migration Checklist

### Local → LAN (move Site B to second machine)
- [ ] Copy Bridge B config, update `urbit.url` to point to remote ship
- [ ] Run Gateway B on second machine, update inter-gateway link to LAN IP
- [ ] Update Gateway A to send to LAN IP instead of localhost
- [ ] Verify bidirectional communication over real network

### LAN → Helium (production)
- [ ] Purchase Helium OUI ($235)
- [ ] Register both DevAddr ranges on Helium routes
- [ ] Point routes to Gateway A and Gateway B public IPs
- [ ] Remove inter-gateway direct link — Helium routes between them
- [ ] Verify bidirectional communication over Helium
