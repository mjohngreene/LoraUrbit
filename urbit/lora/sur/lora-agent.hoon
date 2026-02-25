::  sur/lora-agent.hoon — Type definitions for %lora-agent
::
::  Defines the core types for LoraUrbit: uplink packets, device state,
::  peer-to-peer messaging, poke actions, and subscription updates.
::  These types are the contract between the Rust bridge (which sends
::  JSON pokes) and the Gall agent (which stores and routes data).
::
|%
::  +packet-source: where the packet originated
::
+$  packet-source
  $?  %local     ::  direct from a local LoRa gateway via Semtech UDP
      %helium    ::  routed through the Helium Network via OUI
  ==
::
::  +mtype: LoRaWAN message type (from MHDR)
::
+$  mtype
  $?  %join-request
      %join-accept
      %unconfirmed-data-up
      %unconfirmed-data-down
      %confirmed-data-up
      %confirmed-data-down
      %rejoin-request
      %proprietary
  ==
::
::  +uplink: a decoded LoRaWAN uplink packet
::
::  This is what the Rust bridge sends us after decoding a raw
::  LoRaWAN PHY payload from the UDP server.
::
+$  uplink
  $:  dev-addr=@t        ::  device address, hex string e.g. "01abcdef"
      fcnt=@ud            ::  frame counter
      f-port=(unit @ud)   ::  application port (~ if not present)
      payload=@t          ::  application payload, hex encoded
      rssi=@rs            ::  RSSI in dBm (single-precision float)
      snr=(unit @rs)      ::  signal-to-noise ratio
      freq=@t             ::  frequency in MHz, string e.g. "902.3"
      data-rate=@t        ::  data rate string e.g. "SF7BW125"
      gateway-eui=@t      ::  EUI of receiving gateway
      received-at=@da     ::  timestamp of reception
      =mtype              ::  LoRaWAN message type
      source=packet-source
  ==
::
::  +device: a registered LoRa device
::
::  Maintained by the agent. Updated each time an uplink arrives.
::
+$  device
  $:  dev-addr=@t         ::  device address
      name=(unit @t)      ::  human-readable name
      description=(unit @t)
      last-seen=@da       ::  timestamp of last uplink
      packet-count=@ud    ::  total uplinks received
  ==
::
::  +downlink: a pending downlink command to a device
::
+$  downlink
  $:  dev-addr=@t         ::  target device address
      f-port=@ud          ::  application port
      payload=@t          ::  hex encoded payload
      confirmed=?          ::  confirmed or unconfirmed
      queued-at=@da       ::  when the downlink was requested
      sent=?               ::  has the bridge picked this up?
  ==
::
::  === Peer-to-peer messaging types (Phase 3c) ===
::
::  +peer-status: whether a peer is reachable
::
+$  peer-status  ?(%online %offline)
::
::  +peer: a known peer ship with its LoRa device address
::
+$  peer
  $:  =ship
      dev-addr=@t
      last-seen=@da
      status=peer-status
  ==
::
::  +outbound-msg: a message queued for transmission via LoRa
::
+$  outbound-msg
  $:  id=@ud
      dest-ship=@p
      dest-addr=@t
      payload=@t
      queued-at=@da
      sent=?
  ==
::
::  +inbound-msg: a message received from the LoRa network
::
+$  inbound-msg
  $:  id=@ud
      src-ship=(unit @p)
      src-addr=@t
      payload=@t
      received-at=@da
  ==
::
::  +action: poke actions accepted by %lora-agent
::
::  The Rust bridge pokes us with %uplink when it decodes a packet.
::  Users/agents poke with %register-device or %downlink-request.
::  Phase 3c adds peer-to-peer messaging actions.
::
+$  action
  $%  [%uplink =uplink]
      $:  %register-device
          dev-addr=@t
          name=(unit @t)
          description=(unit @t)
      ==
      $:  %downlink-request
          dev-addr=@t
          f-port=@ud
          payload=@t
          confirmed=?
      ==
      [%downlink-ack dev-addr=@t success=?]
      ::  peer-to-peer actions
      [%register-peer =ship dev-addr=@t]
      [%send-message dest=@p payload=@t]
      [%message-received src-addr=@t payload=@t]
      [%set-identity dev-addr=@t]
      [%tx-ack msg-id=@ud]
      [%tx-fail msg-id=@ud]
  ==
::
::  +update: subscription updates sent to watchers
::
+$  update
  $%  [%new-uplink =uplink]
      [%device-update =device]
      [%downlink-sent dev-addr=@t success=?]
      [%initial-devices devices=(list device)]
      ::  peer-to-peer updates
      [%peer-registered =peer]
      [%message-queued =outbound-msg]
      [%message-sent msg-id=@ud]
      [%message-failed msg-id=@ud]
      [%new-message =inbound-msg]
  ==
--
