# Phase 3d — End-to-End Integration Test Results

**Date:** 2026-02-26
**Environment:** Mac mini (arm64), all processes on localhost

## Topology

```
~zod (%lora-agent)                          ~bus (%lora-agent)
    ↕ Airlock (localhost:8080)                  ↕ Airlock (localhost:8081)
Bridge A (UDP 1680)                         Bridge B (UDP 1681)
    ↕ Semtech UDP                               ↕ Semtech UDP
Gateway A (UDP 1700)                        Gateway B (UDP 1701)
    └──────── localhost UDP link ───────────────┘
```

## Components

| Component | tmux session | Config |
|-----------|-------------|--------|
| ~zod (fakezod) | `zod` | port 8080, +code: `lidlut-tabwed-pillex-ridrup` |
| ~bus (fakebus) | `bus` | port 8081, +code: `riddec-bicrym-ridlev-pocsef` |
| Gateway Pair | `gateway-pair` | A=1700, B=1701 |
| Bridge A | `bridge-a` | config-a.toml (UDP 1680 → ~zod) |
| Bridge B | `bridge-b` | config-b.toml (UDP 1681 → ~bus) |

## Test 1: ~zod → ~bus ("Hello")

**Setup:**
```
~zod:  :lora-agent &json '{"action":"set-identity","dev-addr":"260B1234"}'
~zod:  :lora-agent &json '{"action":"register-peer","ship":"~bus","dev-addr":"01AB5678"}'
~bus:  :lora-agent &json '{"action":"set-identity","dev-addr":"01AB5678"}'
~bus:  :lora-agent &json '{"action":"register-peer","ship":"~zod","dev-addr":"260B1234"}'
```

**Send:**
```
~zod:  :lora-agent &json '{"action":"send-message","dest":"~bus","payload":"48656C6C6F"}'
```

**Result: ✅ SUCCESS**

~bus inbox entry:
```json
{
    "received-at": 1772123360,
    "src-ship": "~zod",
    "src-addr": "260B1234",
    "id": 5,
    "payload": "48656c6c6f"
}
```

Flow verified: Bridge A polled outbox → built LoRaWAN frame with ~zod's DevAddr (260B1234) → PULL_RESP to Gateway A → relayed to Gateway B → PUSH_DATA to Bridge B → decoded uplink → Hoon agent resolved DevAddr to ~zod → routed to inbox.

## Test 2: ~bus → ~zod ("World")

**Send:**
```
~bus:  :lora-agent &json '{"action":"send-message","dest":"~zod","payload":"576F726C64"}'
```

**Result: ✅ SUCCESS**

~zod inbox entry:
```json
{
    "received-at": 1772123453,
    "src-ship": "~bus",
    "src-addr": "01AB5678",
    "id": 11,
    "payload": "576f726c64"
}
```

Bidirectional communication confirmed.

## Final State

### ~zod
```json
{
    "inbox-count": 3,
    "peer-count": 1,
    "device-count": 5,
    "outbox-count": 9,
    "uplink-count": 10
}
```
Peers: `[{"ship": "~bus", "dev-addr": "01AB5678", "status": "online"}]`
Outbox: `[]` (all messages transmitted)

### ~bus
```json
{
    "inbox-count": 8,
    "peer-count": 1,
    "device-count": 2,
    "outbox-count": 1,
    "uplink-count": 7
}
```
Peers: `[{"ship": "~zod", "dev-addr": "260B1234", "status": "online"}]`
Outbox: `[]` (all messages transmitted)

## Bugs Found & Fixed During Testing

### 1. Peer resolution on uplink (Hoon)
**Problem:** The `%uplink` poke handler didn't check if the DevAddr belonged to a registered peer. Messages arrived as raw uplinks without being routed to the inbox.

**Fix:** Enhanced the `%uplink` handler to look up the sender's DevAddr in the peer map. If found, the uplink is automatically routed to the inbox as an `inbound-msg` with the sender's `@p` identity resolved.

### 2. Source DevAddr in outbox scry (Hoon)
**Problem:** The `/outbox` scry only included `dest-addr` (recipient). The bridge had no way to know the *sender's* DevAddr to put in the LoRaWAN frame header.

**Fix:** Added `src-addr` field to the outbox scry output, populated from `my-addr` state.

### 3. LoRaWAN frame uses sender's DevAddr (Rust)
**Problem:** The bridge was putting the *destination* DevAddr in the LoRaWAN frame header. The receiving bridge then couldn't identify who sent the message.

**Fix:** Changed `run_outbound_task()` to use `src_addr` (sender's own DevAddr) in the frame header, falling back to `dest_addr` if not set.

### 4. Flexible queued_at parsing (Rust)
**Problem:** `OutboundMessage.queued_at` expected a string, but Urbit's `sect:enjs:format` outputs a number (unix timestamp).

**Fix:** Changed type to `serde_json::Value` to accept both.

## Conclusion

**Phase 3 is complete.** Two Urbit ships can exchange bidirectional messages over simulated LoRa gateways with identity resolution. The full stack works:

1. Ship queues message via `%send-message` poke
2. Bridge polls outbox via scry, builds LoRaWAN frame
3. Bridge sends PULL_RESP to gateway
4. Gateway pair relays packet to the other side
5. Receiving bridge decodes uplink, pokes ship
6. Ship's agent resolves sender identity from DevAddr → peer map
7. Message lands in inbox with sender `@p` attributed

No Ames dependency for data transport. Identity-aware. Sovereign.
