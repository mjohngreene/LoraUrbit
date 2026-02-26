# Phase 3d — End-to-End Integration Test Results

**Date:** 2026-02-26 (verified)
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
| Gateway Pair | `gateway-pair` | GW_A=0.0.0.0:1700, GW_B=0.0.0.0:1701 |
| Bridge A | `bridge-a` | config-a.toml (UDP 1680 → ~zod at localhost:8080) |
| Bridge B | `bridge-b` | config-b.toml (UDP 1681 → ~bus at localhost:8081) |

## Peer Registration

```
~zod:  :lora-agent &json '{"action":"set-identity","dev-addr":"260B1234"}'
~zod:  :lora-agent &json '{"action":"register-peer","ship":"~bus","dev-addr":"01AB5678"}'
~bus:  :lora-agent &json '{"action":"set-identity","dev-addr":"01AB5678"}'
~bus:  :lora-agent &json '{"action":"register-peer","ship":"~zod","dev-addr":"260B1234"}'
```

## Test 1: ~zod → ~bus ("Hello")

**Send (from ~zod dojo):**
```
:lora-agent &json '{"action":"send-message","dest":"~bus","payload":"48656C6C6F"}'
```

### Observed Flow

1. **~zod dojo:** `"lora-agent: queued message 13 for ~bus"`
2. **Bridge A:** `Outbox has 1 pending message(s)` → `Processing outbound msg #13: dest=~bus (01AB5678) payload=48656C6C6F`
3. **Bridge A:** `Sent PULL_RESP to gateway 127.0.0.1:1700 (token=0x5c78, 180 bytes)`
4. **Gateway Pair:** `[GW-A] 📩 PULL_RESP (downlink) from 127.0.0.1:1680 (180 bytes)` → `[GW-A] 📤 Downlink relayed as uplink to peer bridge 127.0.0.1:1681`
5. **Bridge B:** `PUSH_DATA from gateway bb00000000000002` → `LoRaWAN: UnconfirmedDataDown DevAddr=260B1234 FCnt=0 FPort=1 Payload=5 bytes`
6. **Bridge B:** `Poked %lora-agent with uplink from 260B1234`
7. **~bus dojo:** `"lora-agent: uplink from '260B1234'"` → `"lora-agent: peer uplink from ~zod ('260B1234') payload='48656c6c6f'"`
8. **~zod dojo:** `"lora-agent: tx-ack for message 13"` (confirming TX)

### Result: ✅ SUCCESS

**~bus inbox (verified via scry):**
```json
{
    "received-at": 1772123531,
    "src-ship": "~zod",
    "src-addr": "260B1234",
    "id": 9,
    "payload": "48656c6c6f"
}
```

Payload decoded: `48656c6c6f` → **"Hello"** ✅

## Test 2: ~bus → ~zod ("World")

**Send (from ~bus dojo):**
```
:lora-agent &json '{"action":"send-message","dest":"~zod","payload":"576F726C64"}'
```

### Observed Flow

1. **~bus dojo:** message queued, `"lora-agent: tx-ack for message 10"`
2. **Bridge B:** `Processing outbound msg #10` → `Sent PULL_RESP to gateway 127.0.0.1:1701`
3. **Gateway Pair:** `[GW-B] 📩 PULL_RESP (downlink) from 127.0.0.1:1681` → `[GW-B] 📤 Downlink relayed as uplink to peer bridge 127.0.0.1:1680`
4. **Bridge A:** `LoRaWAN: UnconfirmedDataDown DevAddr=01AB5678 FCnt=0 FPort=1 Payload=5 bytes`
5. **Bridge A:** `Poked %lora-agent with uplink from 01AB5678`
6. **~zod dojo:** `"lora-agent: peer uplink from ~bus ('01AB5678') payload='576f726c64'"`

### Result: ✅ SUCCESS

**~zod inbox (verified via scry):**
```json
{
    "received-at": 1772123588,
    "src-ship": "~bus",
    "src-addr": "01AB5678",
    "id": 14,
    "payload": "576f726c64"
}
```

Payload decoded: `576f726c64` → **"World"** ✅

## Scry Verification (Post-Test)

### ~zod /stats
```json
{
    "inbox-count": 4,
    "peer-count": 1,
    "device-count": 5,
    "outbox-count": 10,
    "uplink-count": 11
}
```

### ~zod /peers
```json
[{"ship": "~bus", "dev-addr": "01AB5678", "status": "online", "last-seen": 1772123588}]
```

### ~zod /outbox
```json
[]
```

### ~bus /stats
```json
{
    "inbox-count": 9,
    "peer-count": 1,
    "device-count": 2,
    "outbox-count": 2,
    "uplink-count": 8
}
```

### ~bus /peers
```json
[{"ship": "~zod", "dev-addr": "260B1234", "status": "online", "last-seen": 1772123531}]
```

### ~bus /outbox
```json
[]
```

## Bugs Found & Fixed During Integration Testing

### 1. OutboundMessage deserialization failed silently (Rust)
**Problem:** `OutboundMessage.queued_at` was typed as `String`, but Urbit's `sect:enjs:format` outputs a bare number (unix timestamp). `serde_json::from_value` failed, the error was caught by a generic `Err(_) => continue`, and the bridge silently skipped all outbox messages.

**Fix:** Changed `queued_at` type to `serde_json::Value` to accept both string and number.

### 2. LoRaWAN frame used destination DevAddr instead of source (Rust)
**Problem:** The bridge built LoRaWAN frames with `dev_addr = msg.dest_addr` (the recipient's address). The receiving bridge decoded DevAddr=01AB5678 (itself) and couldn't identify the sender.

**Fix:** Changed `run_outbound_task()` to use `msg.src_addr` (sender's own DevAddr from the `/outbox` scry) in the LoRaWAN frame header. Receiver now sees the sender's DevAddr and resolves identity from the peer map.

### 3. Hoon uplink handler didn't route peer messages to inbox
**Problem:** The `%uplink` poke handler only tracked devices (packet count, last-seen). It didn't check if the DevAddr belonged to a registered peer, so peer messages were lost — they incremented uplink-count but never reached the inbox.

**Fix:** Enhanced the `%uplink` handler to look up DevAddr in the peer map. If the DevAddr matches a registered peer, the handler extracts the `payload` field from the uplink JSON, resolves the sender's `@p`, and routes the message to the inbox as an `inbound-msg`.

### 4. Double-poke from bridge created duplicate/malformed inbox entries
**Problem:** The bridge was poking both `%uplink` (device tracking) and `%message-received` (P2P) for every received packet. After fixing bug #3, the uplink handler already routes peer messages to inbox, so the `%message-received` poke was redundant and sometimes malformed (attempted to parse sender from payload bytes).

**Fix:** Removed the `%message-received` poke from the bridge's airlock task. Peer detection is now handled entirely by the Hoon agent's `%uplink` handler.

### 5. Sender's DevAddr not included in outbox scry (Hoon)
**Problem:** The `/outbox` scry only returned `dest-addr`. The bridge needed the sender's DevAddr to put in the LoRaWAN frame.

**Fix:** Added `src-addr` to the outbox scry, populated from the agent's `my-addr` state (set via `%set-identity`).

### 6. Timing race: bridge polls before gateway keepalive
**Observation:** The bridge starts polling the outbox immediately on startup, but the gateway pair only sends its first PULL_DATA keepalive after 10 seconds. If a message is in the outbox during this window, the bridge fails with "no gateway address known" and pokes `tx-fail`, removing the message.

**Mitigation:** The message was re-sent after the gateway keepalive arrived. In production, the retry logic should be improved (e.g., delay outbox polling until gateway address is known, or re-queue failed messages).

## Architecture Notes

### Message Flow (proven)
```
~zod sends "Hello" to ~bus:

  Dojo: :lora-agent &json '{"action":"send-message","dest":"~bus","payload":"48656C6C6F"}'
    ↓ (poke → outbox queue)
  Bridge A: scry /outbox → msg found → build LoRaWAN frame (DevAddr=260B1234, FPort=1, FRMPayload="Hello")
    ↓ (PULL_RESP to gateway)
  Gateway A: relay PUSH_DATA to Bridge B (via Gateway B)
    ↓ (Semtech UDP relay)
  Bridge B: decode LoRaWAN frame → extract DevAddr=260B1234, payload=48656c6c6f
    ↓ (poke %uplink)
  ~bus agent: DevAddr 260B1234 → peer lookup → ~zod → route to inbox
    ↓
  ✅ Message in inbox: {src-ship: "~zod", payload: "48656c6c6f"}
```

### Key Design Decision: DevAddr = Sender Identity
The LoRaWAN frame's DevAddr field carries the **sender's** address, not the receiver's. This allows the receiving side to identify who sent the message through the peer map. Standard LoRaWAN uses DevAddr for device-to-network addressing, but in our P2P overlay it serves as a lightweight sender identity.

### No Ames Required
All data transport happens over simulated LoRa radio (UDP on localhost). Urbit's Ames protocol is not used for message delivery. The Hoon agent is fully self-contained — it manages peers, outbox, inbox, and identity using only JSON pokes and scry endpoints.

## Conclusion

**Phase 3 is complete.** Bidirectional peer-to-peer messaging between two Urbit ships over simulated LoRa gateways is verified. The full stack — Hoon agent, Rust bridge, Semtech GWMP, gateway pair simulator — works end-to-end with correct sender identity resolution.

Hoon owns the brains. Rust owns the wire. ✅
