# Semtech UDP Packet Forwarder Protocol

## Overview

The Semtech UDP Packet Forwarder protocol (also called GWMP — Gateway Messaging Protocol) is the de facto standard for communication between LoRa gateways and network servers. It's a simple binary+JSON protocol over UDP.

Reference: https://github.com/Lora-net/packet_forwarder/blob/master/PROTOCOL.TXT

## Packet Types

All packets share a common 4-byte header:

```
Byte 0: Protocol version (always 0x02)
Byte 1-2: Random token (big-endian u16)
Byte 3: Packet type identifier
```

### PUSH_DATA (0x00) — Gateway → Server
Gateway sends received RF packets and/or status.

```
[version:1][token:2][0x00][gateway_eui:8][json_payload]
```

JSON payload contains `rxpk` (received packets) and/or `stat` (gateway status):
```json
{
  "rxpk": [{
    "freq": 902.3,
    "rssi": -65,
    "lsnr": 7.5,
    "datr": "SF7BW125",
    "codr": "4/5",
    "size": 15,
    "data": "<base64 PHY payload>"
  }],
  "stat": {
    "time": "2026-02-18 12:00:00 UTC",
    "rxnb": 47,
    "rxok": 44
  }
}
```

### PUSH_ACK (0x01) — Server → Gateway
Acknowledgment. Must echo the same random token.

```
[version:1][token:2][0x01]
```

### PULL_DATA (0x02) — Gateway → Server
Keepalive and NAT traversal. Gateway sends these periodically.

```
[version:1][token:2][0x02][gateway_eui:8]
```

### PULL_ACK (0x04) — Server → Gateway
Acknowledgment of PULL_DATA.

```
[version:1][token:2][0x04]
```

### PULL_RESP (0x03) — Server → Gateway
Server sends a downlink packet to the gateway for transmission.

```
[version:1][token:2][0x03][json_payload]
```

### TX_ACK (0x05) — Gateway → Server
Gateway confirms downlink transmission result.

```
[version:1][token:2][0x05][gateway_eui:8][optional_json]
```

## LoRaWAN PHY Payload

The `data` field in `rxpk` is a base64-encoded LoRaWAN PHY payload:

```
[MHDR:1][MACPayload:variable][MIC:4]
```

### MHDR (MAC Header)
```
Bits 7-5: MType (message type)
  000 = Join Request
  001 = Join Accept
  010 = Unconfirmed Data Up
  011 = Unconfirmed Data Down
  100 = Confirmed Data Up
  101 = Confirmed Data Down
  110 = Rejoin Request
  111 = Proprietary

Bits 4-2: RFU (reserved)
Bits 1-0: Major version (00 = LoRaWAN R1)
```

### Data Frame MACPayload
```
[DevAddr:4 LE][FCtrl:1][FCnt:2 LE][FOpts:0-15][FPort:0-1][FRMPayload:variable]
```

### FCtrl (uplink)
```
Bit 7: ADR
Bit 6: ADRACKReq
Bit 5: ACK
Bit 4: ClassB / FPending
Bits 3-0: FOptsLen
```
