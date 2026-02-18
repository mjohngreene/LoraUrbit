# LoraUrbit Architecture

## Overview

LoraUrbit bridges two networks:
1. **LoRaWAN** — low-power, wide-area radio for IoT sensors
2. **Urbit (Ames)** — cryptographic, identity-native, E2E encrypted networking

The bridge replaces the traditional HTTP/MQTT application transport layer in LoRaWAN with Urbit's Ames protocol, creating sovereign IoT infrastructure.

## Data Flow

### Local Gateway Path
```
Sensor → (RF 902-928MHz) → LoRa Gateway → (Semtech UDP :1680) → LoraUrbit → (Airlock HTTP) → Urbit Ship → (Ames) → Subscribers
```

### Helium Network Path
```
Sensor → (RF) → Helium Hotspot → Helium Packet Router → (GWMP UDP) → LoraUrbit → (Airlock HTTP) → Urbit Ship → (Ames) → Subscribers
```

Key insight: **Both paths converge at the same UDP server.** The Helium Packet Router speaks the same Semtech GWMP protocol as a local gateway, so our UDP server handles both transparently.

## Components

### 1. UDP Server (`src/udp/`)
- Listens on UDP port 1680 (configurable)
- Speaks Semtech Gateway Messaging Protocol (GWMP)
- Handles: PUSH_DATA (uplinks), PULL_DATA (keepalive), TX_ACK (downlink status)
- Responds: PUSH_ACK, PULL_ACK
- Passes decoded rxpk payloads to LoRaWAN decoder

### 2. LoRaWAN Decoder (`src/lorawan/`)
- Parses PHY payload from base64
- Decodes MAC header (MHDR): message type, major version
- Decodes frame: DevAddr, FCtrl, FCnt, FOpts, FPort, FRMPayload, MIC
- Handles: JoinRequest, JoinAccept, Data Up/Down, Proprietary
- Phase 4: MIC verification using NwkSKey, payload decryption using AppSKey

### 3. Urbit Bridge (`src/urbit/`)
- Connects to local Urbit ship via Airlock (Eyre HTTP API)
- Authenticates using +code
- Pokes %lora-agent with decoded packets as JSON
- Subscribes to paths for downlink commands and device management

### 4. Helium Integration (`src/helium/`)
- OUI registration and route management via Config Service (gRPC)
- DevAddr slab management
- Data Credit monitoring
- Phase 4 MVP: GWMP mode (reuses UDP server)
- Phase 5: Native Packet Router gRPC streaming

### 5. Gall Agent (`urbit/lora-agent/`)
- Hoon application running inside Urbit
- Receives pokes from the bridge with uplink data
- Maintains device state in Urbit's persistent store
- Publishes on subscription paths (e.g., `/lora/devices`, `/lora/uplinks`)
- Remote ships subscribe over Ames — data flows encrypted, ship-to-ship

## Protocol Stack Comparison

### Traditional LoRaWAN
```
Application ←── HTTP/MQTT ──→ Application Server
    ↑                              ↑
Network Server ←── TCP/TLS ──→ Cloud Infrastructure
    ↑
Gateway ←── Semtech UDP ──→ (local network)
    ↑
End Device ←── LoRa RF ──→ (sub-GHz ISM band)
```

### LoraUrbit
```
Subscriber Ship ←── Ames (E2E encrypted) ──→ Your Ship (%lora-agent)
                                                  ↑
                                            Airlock HTTP (localhost)
                                                  ↑
                                            LoraUrbit Bridge (Rust)
                                                  ↑
Gateway / Helium ←── Semtech UDP ──→ (local network)
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

## Helium Integration Details

### OUI Setup ($235 one-time)
1. Create Helium wallet (Solana format)
2. Generate owner + delegate keypairs via `helium-config-service-cli`
3. Email Helium Foundation to purchase OUI + DevAddr slab
4. Fund escrow with minimum 3.5M Data Credits ($35)

### Route Configuration
- Register LoraUrbit's public IP as the LNS endpoint
- Configure GWMP port mapping (e.g., US915 → 1701)
- Add DevAddr ranges for routing uplinks
- Add Device EUI pairs for routing join requests

### Packet Flow
1. Device sends uplink
2. Nearest Helium hotspot picks it up
3. Helium Packet Router matches DevAddr to our OUI route
4. Packet Router forwards to our IP via GWMP (Semtech UDP)
5. Our UDP server receives it — identical to a local gateway packet
6. Decoded and bridged to Urbit as usual
