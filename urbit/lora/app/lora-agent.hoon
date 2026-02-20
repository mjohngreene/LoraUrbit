::  app/lora-agent.hoon — LoraUrbit Gall Agent
::
::  Sovereign LoRaWAN device management powered by Urbit.
::  Accepts JSON pokes from the Rust bridge via Airlock.
::
/+  default-agent, dbug
|%
+$  card  card:agent:gall
::
+$  device
  $:  dev-addr=@t
      name=(unit @t)
      last-seen=@da
      packet-count=@ud
  ==
::
+$  state-0
  $:  %0
      devices=(map @t device)
      uplink-count=@ud
  ==
--
%-  agent:dbug
=|  state-0
=*  state  -
^-  agent:gall
|_  =bowl:gall
+*  this  .
    def   ~(. (default-agent this %.n) bowl)
::
++  on-init
  ^-  (quip card _this)
  ~&  >  "lora-agent: initialized"
  `this
::
++  on-save  !>(state)
::
++  on-load
  |=  old-vase=vase
  ^-  (quip card _this)
  ~&  >  "lora-agent: reloaded"
  =/  old  !<(state-0 old-vase)
  `this(state old)
::
++  on-poke
  |=  [=mark =vase]
  ^-  (quip card _this)
  ?+  mark  (on-poke:def mark vase)
      %json
    =/  jon=json  !<(json vase)
    ?.  ?=([%o *] jon)
      ~&  >>>  "lora-agent: expected JSON object"
      `this
    =/  obj  p.jon
    =/  action-type=(unit json)  (~(get by obj) 'action')
    ?~  action-type
      ~&  >>>  "lora-agent: missing 'action' field"
      `this
    ?.  ?=([%s *] u.action-type)
      ~&  >>>  "lora-agent: 'action' must be a string"
      `this
    =/  act=@t  p.u.action-type
    ?+  act
      ~&  >>>  "lora-agent: unknown action {<act>}"
      `this
    ::
        %'uplink'
      ::  extract dev-addr from JSON
      =/  dev-addr=@t
        =/  val  (~(got by obj) 'dev-addr')
        ?>  ?=([%s *] val)
        p.val
      ~&  >  "lora-agent: uplink from {<dev-addr>}"
      ::  update or create device entry
      =/  dev=device
        =/  existing  (~(get by devices) dev-addr)
        ?~  existing
          [dev-addr ~ now.bowl 1]
        u.existing(last-seen now.bowl, packet-count +(packet-count.u.existing))
      =.  devices  (~(put by devices) dev-addr dev)
      =.  uplink-count  +(uplink-count)
      ::  notify subscribers
      =/  upd=json
        %-  pairs:enjs:format
        :~  ['type' s+'new-uplink']
            ['dev-addr' s+dev-addr]
        ==
      :_  this
      :~  [%give %fact ~[/uplinks] %json !>(upd)]
          [%give %fact ~[/devices] %json !>(upd)]
      ==
    ::
        %'subscribe-remote'
      ::  subscribe to a remote ship's lora-agent
      =/  target-ship=@p
        =/  val  (~(got by obj) 'ship')
        ?>  ?=([%s *] val)
        (slav %p p.val)
      =/  sub-path=path
        =/  val  (~(got by obj) 'path')
        ?>  ?=([%s *] val)
        (stab p.val)
      ~&  >  "lora-agent: subscribing to {<target-ship>} on {<sub-path>}"
      :_  this
      :~  [%pass /remote-uplinks %agent [target-ship %lora-agent] %watch sub-path]
      ==
    ::
        %'register-device'
      =/  dev-addr=@t
        =/  val  (~(got by obj) 'dev-addr')
        ?>  ?=([%s *] val)
        p.val
      =/  name=(unit @t)
        =/  val  (~(get by obj) 'name')
        ?~  val  ~
        ?.  ?=([%s *] u.val)  ~
        (some p.u.val)
      ~&  >  "lora-agent: registering {<dev-addr>}"
      =/  dev=device
        =/  existing  (~(get by devices) dev-addr)
        ?~  existing
          [dev-addr name now.bowl 0]
        u.existing(name name)
      =.  devices  (~(put by devices) dev-addr dev)
      =/  upd=json
        %-  pairs:enjs:format
        :~  ['type' s+'device-registered']
            ['dev-addr' s+dev-addr]
        ==
      :_  this
      :~  [%give %fact ~[/devices] %json !>(upd)]
      ==
    ==
  ==
::
++  on-watch
  |=  =path
  ^-  (quip card _this)
  ?+  path  (on-watch:def path)
      [%uplinks ~]
    ~&  >  "lora-agent: subscriber on /uplinks"
    `this
  ::
      [%devices ~]
    ~&  >  "lora-agent: subscriber on /devices"
    `this
  ==
::
++  on-leave
  |=  =path
  ^-  (quip card _this)
  `this
::
++  on-peek
  |=  =path
  ^-  (unit (unit cage))
  ?+  path  (on-peek:def path)
      [%x %stats ~]
    =/  result=json
      %-  pairs:enjs:format
      :~  ['device-count' (numb:enjs:format ~(wyt by devices))]
          ['uplink-count' (numb:enjs:format uplink-count)]
      ==
    ``json+!>(result)
  ::
      [%x %devices ~]
    =/  dev-list=(list [@t device])  ~(tap by devices)
    =/  result=json
      :-  %a
      %+  turn  dev-list
      |=  [key=@t dev=device]
      %-  pairs:enjs:format
      :~  ['dev-addr' s+dev-addr.dev]
          ['name' ?~(name.dev ~ s+u.name.dev)]
          ['last-seen' (sect:enjs:format last-seen.dev)]
          ['packet-count' (numb:enjs:format packet-count.dev)]
      ==
    ``json+!>(result)
  ==
::
++  on-agent
  |=  [=wire =sign:agent:gall]
  ^-  (quip card _this)
  ?+  wire  (on-agent:def wire sign)
      [%remote-uplinks ~]
    ?+  -.sign  (on-agent:def wire sign)
        %fact
      =/  jon=json  !<(json q.cage.sign)
      ~&  >  "lora-agent: received remote uplink: {<jon>}"
      `this
    ::
        %watch-ack
      ?~  p.sign
        ~&  >  "lora-agent: remote subscription confirmed"
        `this
      ~&  >>>  "lora-agent: remote subscription failed"
      `this
    ::
        %kick
      ~&  >  "lora-agent: remote subscription kicked, resubscribing..."
      :_  this
      :~  [%pass /remote-uplinks %agent [src.bowl %lora-agent] %watch /uplinks]
      ==
    ==
  ==
::
++  on-arvo
  |=  [=wire =sign-arvo]
  ^-  (quip card _this)
  (on-arvo:def wire sign-arvo)
::
++  on-fail
  |=  [=term =tang]
  ^-  (quip card _this)
  (on-fail:def term tang)
--
