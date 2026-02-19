::  lib/lora.hoon — LoraUrbit helper library
::
::  Shared utilities for LoRa data processing.
::  Currently minimal; will grow as needed.
::
/-  *lora-agent
|%
::
::  +dev-addr-to-cord: normalize a device address to lowercase hex
::
++  dev-addr-to-cord
  |=  addr=@t
  ^-  @t
  (crip (cass (trip addr)))
::
::  +summarize-device: produce a one-line summary of a device
::
++  summarize-device
  |=  dev=device
  ^-  @t
  =/  n  ?~(name.dev dev-addr.dev u.name.dev)
  (crip "{(trip n)}: {<packet-count.dev>} packets, last seen {<last-seen.dev>}")
--
