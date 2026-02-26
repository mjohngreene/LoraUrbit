# Phase 3d — End-to-End Integration Test Results

**Date:** 2026-02-26
**Status:** ✅ PASS — Bidirectional peer-to-peer messaging verified

## Architecture

```
~zod (%lora-agent)                          ~bus (%lora-agent)
    ↕ Airlock (localhost:8080)                  ↕ Airlock (localhost:8081)
Bridge A (UDP 1680)                         Bridge B (UDP 1681)
    ↕ Semtech UDP                               ↕ Semtech UDP
Gateway A (UDP 1700)                        Gateway B (UDP 1701)
    └──────── localhost UDP link ───────────────┘
```

## Identity Configuration

| Ship | DevAddr    | Role    |
|------|-----------|---------|
| ~zod | 260B1234  | Sender/Receiver |
| ~bus | 01AB5678  | Sender/Receiver |

## Test 1: ~zod → ~bus ("Hello")

**Command on ~zod:**
```
:lora-agent &json '{"action":"send-message","dest":"~bus","payload":"48656C6C6F"}'
```

**~bus inbox result:**
```json
{
    "received-at": 1772123360,
    "src-ship": "~zod",
    "src-addr": "260B1234",
    "id": 5,
    "payload": "48656c6c6f"
}
```

**Result:** ✅ Message received with correct sender identity resolved via peer registry.

## Test 2: ~bus → ~zod ("World")

**Command on ~bus:**
```
:lora-agent &json '{"action":"send-message","dest":"~zod","payload":"576F726C64"}'
```

**~zod inbox result:**
```json
{
    "received-at": 1772123453,
    "src-ship": "~bus",
    "src-addr": "01AB5678",
    "id": 11,
    "payload": "576f726c64"
}
```

**Result:** ✅ Message received with correct sender identity resolved via peer registry.

## Message Flow (verified via logs)

1. **Hoon agent** queues message in outbox with `src-addr` (sender's DevAddr)
2. **Bridge A** polls outbox via scry, builds LoRaWAN frame with:
   - DevAddr = destination's address
   - FRMPayload = [4-byte src-addr prefix] + [application payload]
3. **Bridge A** sends PULL_RESP to Gateway A (Semtech UDP)
4. **Gateway A** relays downlink as PUSH_DATA uplink to Gateway B
5. **Gateway B** forwards PUSH_DATA to Bridge B
6. **Bridge B** decodes LoRaWAN frame, extracts:
   - src-addr from first 4 bytes of FRMPayload
   - payload from remaining bytes
7. **Bridge B** pokes `%lora-agent` with `message-received` action
8. **Hoon agent** resolves src-addr → ship via peer registry, adds to inbox

## Code Changes Made

### Rust bridge (`src/main.rs`)
- Outbound task now prepends sender's DevAddr (4 bytes) to LoRaWAN FRMPayload
- Inbound airlock task now pokes BOTH `uplink` (device tracking) AND `message-received` (P2P inbox)
- Receiving side extracts first 4 bytes of payload as `src-addr`

### Rust types (`src/urbit/types.rs`)
- Added `src_addr` field to `OutboundMessage` struct

### Hoon agent (`urbit/lora/app/lora-agent.hoon`)
- Outbox scry now includes `src-addr` from ship's `my-addr` identity

## Known Issues
- Frames received without the 4-byte src-addr prefix (e.g., from non-P2P sources) will be mis-parsed. Future work: add a protocol version byte or magic prefix to distinguish P2P frames from raw LoRaWAN uplinks.
