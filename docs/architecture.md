# LoraUrbit Architecture

## Design Principle

**Hoon owns the brains. Rust owns the wire.**

All application logic — device state, subscriptions, access control, data routing,
multi-ship peering — lives in Hoon as a Gall agent on your Urbit ship.

Rust handles only what Urbit can't do natively: raw UDP sockets, binary LoRaWAN
protocol parsing, and Helium's gRPC interface. The Rust bridge is intentionally
thin — it decodes packets and hands structured JSON to the ship via Airlock.

## Overview

LoraUrbit bridges two networks:
1. **LoRaWAN** — low-power, wide-area radio for IoT sensors
2. **Urbit (Ames)** — cryptographic, identity-native, E2E encrypted networking

The bridge replaces the traditional HTTP/MQTT application transport layer in
LoRaWAN with Urbit's Ames protocol, creating sovereign IoT infrastructure.

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                        URBIT SHIP                           │
│                                                             │
│  %lora-agent (Gall)              THE BRAINS (Hoon)         │
│  ├── State                                                  │
│  │   ├── device registry    (map @t device)                 │
│  │   ├── packet history     (list uplink)                   │
│  │   └── downlink queue     (list downlink)                 │
│  ├── Poke handlers                                          │
│  │   ├── %lora-action       uplink, register, downlink-req  │
│  │   └── %lora-ack          downlink TX confirmation        │
│  ├── Subscription paths                                     │
│  │   ├── /devices           device registry updates         │
│  │   ├── /uplinks           all uplink packets              │
│  │   └── /uplinks/<addr>    per-device uplink stream        │
│  ├── Scry endpoints                                         │
│  │   ├── /devices           list all known devices          │
│  │   ├── /device/<addr>     single device state             │
│  │   └── /downlink-queue    pending downlinks (for bridge)  │
│  └── Ames peering                                           │
│      └── Remote ships subscribe to paths natively           │
│                                                             │
└──────────────────────┬──────────────────────────────────────┘
                       │ Airlock (HTTP poke/scry, localhost)
                       │
┌──────────────────────┴──────────────────────────────────────┐
│  lora-bridge (Rust)          THE WIRE (thin)                │
│  ├── UDP server         Semtech GWMP (port 1680)            │
│  ├── LoRaWAN decoder    Binary → structured data            │
│  ├── Airlock client     login + poke + scry (~150 LOC)      │
│  └── Helium gRPC        OUI/route management (Phase 4)      │
└──────────────────────┬──────────────────────────────────────┘
                       │ Semtech UDP / GWMP
                       │
         ┌─────────────┴──────────────┐
         │                            │
┌────────┴────────┐    ┌──────────────┴──────────────┐
│  Local LoRa     │    │  Helium Network             │
│  Gateway        │    │  (300k+ hotspots via OUI)   │
└────────┬────────┘    └──────────────┬──────────────┘
         │ RF (902-928MHz)            │ RF
┌────────┴────────┐    ┌──────────────┴──────────────┐
│  LoRa End       │    │  LoRa End                   │
│  Devices        │    │  Devices (anywhere)          │
└─────────────────┘    └─────────────────────────────┘
```

## Data Flow

### Uplink (sensor → ship)
```
1. LoRa device transmits RF packet
2. Gateway (or Helium hotspot) receives, wraps in Semtech UDP
3. Rust bridge receives UDP, decodes LoRaWAN binary payload
4. Rust bridge pokes %lora-agent via Airlock with JSON
5. %lora-agent stores packet, updates device state
6. %lora-agent notifies all subscribers (local + remote ships via Ames)
```

### Downlink (ship → device)
```
1. User (or remote ship) pokes %lora-agent with downlink request
2. %lora-agent queues downlink in state
3. Rust bridge polls /downlink-queue via scry
4. Rust bridge sends PULL_RESP to gateway via UDP
5. Gateway transmits RF to device in next RX window
6. Gateway sends TX_ACK to bridge
7. Bridge pokes %lora-agent with confirmation
```

### Remote subscription (ship-to-ship)
```
1. Remote ship's agent pokes our %lora-agent: [%subscribe /uplinks]
2. %lora-agent adds watcher on path
3. When new uplink arrives, %lora-agent gives update to all watchers
4. Updates flow over Ames — E2E encrypted, identity-authenticated
5. No HTTP, no cloud, no API keys — just Ames
```

## What Lives Where (and Why)

### Must be Rust (Urbit can't do this)

| Component | Reason |
|-----------|--------|
| UDP server (Semtech GWMP) | Urbit has no raw UDP socket support; Eyre is HTTP only |
| LoRaWAN binary decoder | Low-level byte parsing; Hoon can but shouldn't |
| AES-128 MIC/decrypt | Crypto primitives for LoRaWAN security (Phase 4) |
| Helium gRPC client | Protobuf/gRPC is Rust-native in Helium ecosystem |
| Gateway simulator | Testing tool, stays in Rust |

### Must be Hoon (Urbit does this better)

| Component | Reason |
|-----------|--------|
| Device registry | Persistent state — Urbit's event log, no external DB |
| Packet storage | Same — persists automatically, survives restarts |
| Subscription routing | Ames subscriptions are native; building this in Rust duplicates Urbit |
| Access control | Ship identity is the auth — no API keys needed |
| Downlink queue | Agent state, poke-driven, survives restarts |
| Multi-ship peering | Ames is the whole point — sovereign, encrypted, decentralized |
| Web UI | Sail/Landscape, served from the ship itself |

## Protocol Stack Comparison

### Traditional LoRaWAN
```
Application ←── HTTP/MQTT ──→ Application Server (AWS/GCP)
    ↑                              ↑
Network Server ←── TCP/TLS ──→ Cloud Infrastructure
    ↑
Gateway ←── Semtech UDP ──→ (local network)
    ↑
End Device ←── LoRa RF ──→ (sub-GHz ISM band)
```

### LoraUrbit
```
Remote Ships ←── Ames (E2E encrypted) ──→ Your Ship (%lora-agent)
                                                ↑
                                          Airlock (localhost HTTP)
                                                ↑
                                          lora-bridge (Rust, thin)
                                                ↑
Gateway / Helium ←── Semtech UDP ──→ (local network / internet)
    ↑
End Device ←── LoRa RF ──→ (sub-GHz ISM band)
```

## Why Ames Instead of HTTP?

| Property | HTTP/MQTT | Ames |
|----------|-----------|------|
| Identity | DNS + TLS certs (rented) | Urbit ID (owned, on-chain) |
| Encryption | TLS (transport-level) | E2E (application-level) |
| Authentication | API keys / OAuth | Cryptographic ship identity |
| Persistence | Stateless (need a database) | Built into Urbit's event log |
| Routing | Centralized DNS | Decentralized galaxy routing |
| Censorship | Domain can be seized | Ship ID is sovereign |
| Cost | Server hosting + cloud fees | One-time Urbit ID purchase |
