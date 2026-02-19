::  app/lora-agent.hoon — LoraUrbit Gall Agent
::
::  Sovereign LoRaWAN device management powered by Urbit.
::
::  Receives decoded LoRa packets from the Rust bridge via Airlock,
::  stores device state, and publishes updates to subscribers
::  (both local and remote ships over Ames).
::
::  Poke with %lora-action mark.
::  Subscribe to /devices, /uplinks, or /uplinks/<dev-addr>.
::  Scry at /x/devices, /x/device/<addr>, /x/downlink-queue.
::
/-  *lora-agent
|%
+$  versioned-state
  $%  [%0 state-0]
  ==
+$  state-0
  $:  devices=(map @t device)          ::  dev-addr → device
      uplinks=(list uplink)            ::  recent uplink history
      downlink-queue=(list downlink)   ::  pending downlinks
      max-uplinks=@ud                  ::  max uplinks to retain
  ==
--
=|  state=state-0
=*  state  -
%-  agent:dbug
^-  agent:gall
|_  =bowl:gall
+*  this  .
    def   ~(. (default-agent this %.n) bowl)
::
++  on-init
  ^-  (quip card:agent:gall _this)
  ~&  >  "lora-agent: initialized"
  =.  max-uplinks  1.000
  [~ this]
::
++  on-save
  ^-  vase
  !>(state)
::
++  on-load
  |=  old-vase=vase
  ^-  (quip card:agent:gall _this)
  ~&  >  "lora-agent: reloaded"
  =/  old  !<(state-0 old-vase)
  [~ this(state old)]
::
++  on-poke
  |=  [=mark =vase]
  ^-  (quip card:agent:gall _this)
  ?>  ?=(%lora-action mark)
  =/  act  !<(action vase)
  ?-  -.act
  ::
  ::  %uplink: new packet from Rust bridge
  ::
      %uplink
    =/  ul  uplink.act
    ~&  >  "lora-agent: uplink from {<dev-addr.ul>}"
    ::  update or create device entry
    ::
    =/  dev  (~(gut by devices) dev-addr.ul *device)
    =.  dev-addr.dev  dev-addr.ul
    =.  last-seen.dev  received-at.ul
    =.  packet-count.dev  +(packet-count.dev)
    ::  update state
    ::
    =.  devices  (~(put by devices) dev-addr.ul dev)
    =.  uplinks  (snoc uplinks ul)
    ::  trim uplink history if over max
    ::
    =?  uplinks  (gth (lent uplinks) max-uplinks)
      (slag (sub (lent uplinks) max-uplinks) uplinks)
    ::  notify subscribers
    ::
    :_  this
    :~  [%give %fact ~[/uplinks] %lora-update !>(^-([update [%new-uplink ul]]))]
        [%give %fact ~[/uplinks/(scot %t dev-addr.ul)] %lora-update !>(^-([update [%new-uplink ul]]))]
        [%give %fact ~[/devices] %lora-update !>(^-([update [%device-update dev]]))]
    ==
  ::
  ::  %register-device: manually register/name a device
  ::
      %register-device
    ~&  >  "lora-agent: registering device {<dev-addr.act>}"
    =/  dev  (~(gut by devices) dev-addr.act *device)
    =.  dev-addr.dev  dev-addr.act
    =.  name.dev  name.act
    =.  description.dev  description.act
    =.  devices  (~(put by devices) dev-addr.act dev)
    :_  this
    :~  [%give %fact ~[/devices] %lora-update !>(^-([update [%device-update dev]]))]
    ==
  ::
  ::  %downlink-request: queue a downlink command to a device
  ::
      %downlink-request
    ~&  >  "lora-agent: downlink queued for {<dev-addr.act>}"
    =/  dl=downlink
      :*  dev-addr.act
          f-port.act
          payload.act
          confirmed.act
          now.bowl
          %.n
      ==
    =.  downlink-queue  (snoc downlink-queue dl)
    [~ this]
  ::
  ::  %downlink-ack: bridge confirms downlink was sent (or failed)
  ::
      %downlink-ack
    ~&  >  "lora-agent: downlink ack for {<dev-addr.act>} success={<success.act>}"
    ::  remove the oldest matching downlink from queue
    ::
    =.  downlink-queue
      =/  found  %.n
      %+  murn  downlink-queue
      |=  dl=downlink
      ?:  &(!found =(dev-addr.dl dev-addr.act))
        =.  found  %.y
        ~
      (some dl)
    :_  this
    :~  [%give %fact ~[/devices] %lora-update !>(^-([update [%downlink-sent dev-addr.act success.act]]))]
    ==
  ==
::
++  on-watch
  |=  =path
  ^-  (quip card:agent:gall _this)
  ?+  path  (on-watch:def path)
  ::
  ::  /devices: subscribe to device registry updates
  ::  on connect, send current device list
  ::
      [%devices ~]
    ~&  >  "lora-agent: new subscriber on /devices"
    =/  dev-list=(list device)  ~(val by devices)
    :_  this
    :~  [%give %fact ~ %lora-update !>(^-([update [%initial-devices dev-list]]))]
    ==
  ::
  ::  /uplinks: subscribe to all uplink packets
  ::
      [%uplinks ~]
    ~&  >  "lora-agent: new subscriber on /uplinks"
    [~ this]
  ::
  ::  /uplinks/<dev-addr>: subscribe to uplinks from one device
  ::
      [%uplinks @ ~]
    ~&  >  "lora-agent: new subscriber on /uplinks/{<i.t.path>}"
    [~ this]
  ==
::
++  on-leave
  |=  =path
  ^-  (quip card:agent:gall _this)
  ~&  >  "lora-agent: subscriber left {<path>}"
  [~ this]
::
++  on-peek
  |=  =path
  ^-  (unit (unit cage))
  ?+  path  (on-peek:def path)
  ::
  ::  /x/devices — list all known devices
  ::
      [%x %devices ~]
    =/  dev-list=(list device)  ~(val by devices)
    ``!>(dev-list)
  ::
  ::  /x/device/<dev-addr> — single device state
  ::
      [%x %device @ ~]
    =/  addr  i.t.t.path
    =/  dev  (~(get by devices) addr)
    ``!>(dev)
  ::
  ::  /x/uplinks — recent uplink history
  ::
      [%x %uplinks ~]
    ``!>(uplinks)
  ::
  ::  /x/downlink-queue — pending downlinks (for Rust bridge to poll)
  ::
      [%x %downlink-queue ~]
    =/  pending  (skim downlink-queue |=(dl=downlink !sent.dl))
    ``!>(pending)
  ::
  ::  /x/stats — summary statistics
  ::
      [%x %stats ~]
    =/  stats
      :*  device-count=(~(wyt by devices))
          uplink-count=(lent uplinks)
          pending-downlinks=(lent (skim downlink-queue |=(dl=downlink !sent.dl)))
      ==
    ``!>(stats)
  ==
::
++  on-agent
  |=  [=wire =sign:agent:gall]
  ^-  (quip card:agent:gall _this)
  (on-agent:def wire sign)
::
++  on-arvo
  |=  [=wire =sign-arvo]
  ^-  (quip card:agent:gall _this)
  (on-arvo:def wire sign-arvo)
::
++  on-fail
  |=  [=term =tang]
  ^-  (quip card:agent:gall _this)
  (on-fail:def term tang)
--
