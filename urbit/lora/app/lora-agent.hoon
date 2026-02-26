::  app/lora-agent.hoon — LoraUrbit Gall Agent (Phase 3c: Peer-to-Peer)
::
::  Sovereign LoRaWAN peer-to-peer messaging powered by Urbit.
::  Accepts JSON pokes from the Rust bridge via Airlock.
::
::  Each ship runs an identical agent. Ships communicate via LoRa
::  gateways, not via Ames. The bridge polls /outbox for pending
::  messages and pokes with %message-received for inbound ones.
::
/+  default-agent, dbug
|%
+$  card  card:agent:gall
::
+$  peer-status  ?(%online %offline)
::
+$  peer
  $:  =ship
      dev-addr=@t
      last-seen=@da
      status=peer-status
  ==
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
+$  inbound-msg
  $:  id=@ud
      src-ship=(unit @p)
      src-addr=@t
      payload=@t
      received-at=@da
  ==
::
+$  device
  $:  dev-addr=@t
      name=(unit @t)
      last-seen=@da
      packet-count=@ud
  ==
::
::  state-1: peer-to-peer messaging state
::
+$  state-1
  $:  %1
      devices=(map @t device)
      uplink-count=@ud
      peers=(map @p peer)
      my-addr=(unit @t)
      outbox=(list outbound-msg)
      inbox=(list inbound-msg)
      next-msg-id=@ud
  ==
::
::  state-0: previous state for migration
::
+$  state-0
  $:  %0
      devices=(map @t device)
      uplink-count=@ud
  ==
--
%-  agent:dbug
=|  state-1
=*  state  -
^-  agent:gall
|_  =bowl:gall
+*  this  .
    def   ~(. (default-agent this %.n) bowl)
::
++  on-init
  ^-  (quip card _this)
  ~&  >  "lora-agent: initialized (v1 peer-to-peer)"
  `this
::
++  on-save  !>(state)
::
++  on-load
  |=  old-vase=vase
  ^-  (quip card _this)
  ~&  >  "lora-agent: loading state"
  =/  ver  -.q.old-vase
  ?+  ver  `this
    %1
      =/  old  !<(state-1 old-vase)
      `this(state old)
    %0
      ~&  >  "lora-agent: migrating state-0 -> state-1"
      =/  old  !<(state-0 old-vase)
      =/  new=state-1
        :*  %1
            devices.old
            uplink-count.old
            *(map @p peer)
            ~
            *(list outbound-msg)
            *(list inbound-msg)
            0
        ==
      `this(state new)
  ==
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
    ::  === Existing device/uplink actions ===
    ::
        %'uplink'
      =/  dev-addr=@t
        =/  val  (~(got by obj) 'dev-addr')
        ?>  ?=([%s *] val)
        p.val
      ~&  >  "lora-agent: uplink from {<dev-addr>}"
      =/  dev=device
        =/  existing  (~(get by devices) dev-addr)
        ?~  existing
          [dev-addr ~ now.bowl 1]
        u.existing(last-seen now.bowl, packet-count +(packet-count.u.existing))
      =.  devices  (~(put by devices) dev-addr dev)
      =.  uplink-count  +(uplink-count)
      ::  check if this DevAddr belongs to a registered peer
      =/  sender=(unit @p)
        =/  peer-list  ~(tap by peers)
        =/  found=(unit @p)  ~
        |-
        ?~  peer-list  found
        =/  item  i.peer-list
        ?:  =(dev-addr.q.item dev-addr)
          (some p.item)
        $(peer-list t.peer-list)
      ?~  sender
        ::  not a peer — just log the uplink
        =/  upd=json
          %-  pairs:enjs:format
          :~  ['type' s+'new-uplink']
              ['dev-addr' s+dev-addr]
          ==
        :_  this
        :~  [%give %fact ~[/uplinks] %json !>(upd)]
            [%give %fact ~[/devices] %json !>(upd)]
        ==
      ::  peer message! extract payload and route to inbox
      =/  payload=@t
        =/  val  (~(get by obj) 'payload')
        ?~  val  ''
        ?.  ?=([%s *] u.val)  ''
        p.u.val
      ~&  >  "lora-agent: peer uplink from {<u.sender>} ({<dev-addr>}) payload={<payload>}"
      ::  update peer last-seen
      =/  existing  (~(get by peers) u.sender)
      =?  peers  ?=(^ existing)
        (~(put by peers) u.sender u.existing(last-seen now.bowl, status %online))
      ::  add to inbox
      =/  msg=inbound-msg
        :*  next-msg-id
            (some u.sender)
            dev-addr
            payload
            now.bowl
        ==
      =.  inbox  (snoc inbox msg)
      =.  next-msg-id  +(next-msg-id)
      =/  upd=json
        %-  pairs:enjs:format
        :~  ['type' s+'new-message']
            ['id' (numb:enjs:format id.msg)]
            ['src-ship' s+(scot %p u.sender)]
            ['src-addr' s+dev-addr]
            ['payload' s+payload]
        ==
      :_  this
      :~  [%give %fact ~[/uplinks] %json !>(upd)]
          [%give %fact ~[/devices] %json !>(upd)]
          [%give %fact ~[/inbox] %json !>(upd)]
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
      ~&  >  "lora-agent: registering device {<dev-addr>}"
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
    ::
    ::  === Peer-to-peer messaging actions (Phase 3c) ===
    ::
        %'set-identity'
      ::  set this ship's own LoRa DevAddr
      =/  dev-addr=@t
        =/  val  (~(got by obj) 'dev-addr')
        ?>  ?=([%s *] val)
        p.val
      ~&  >  "lora-agent: my DevAddr set to {<dev-addr>}"
      =.  my-addr  (some dev-addr)
      `this
    ::
        %'register-peer'
      ::  associate a ship identity with a LoRa DevAddr
      =/  target-ship=@p
        =/  val  (~(got by obj) 'ship')
        ?>  ?=([%s *] val)
        (slav %p p.val)
      =/  dev-addr=@t
        =/  val  (~(got by obj) 'dev-addr')
        ?>  ?=([%s *] val)
        p.val
      ~&  >  "lora-agent: registered peer {<target-ship>} at {<dev-addr>}"
      =/  p=peer  [target-ship dev-addr now.bowl %online]
      =.  peers  (~(put by peers) target-ship p)
      =/  upd=json
        %-  pairs:enjs:format
        :~  ['type' s+'peer-registered']
            ['ship' s+(scot %p target-ship)]
            ['dev-addr' s+dev-addr]
        ==
      :_  this
      :~  [%give %fact ~[/peers] %json !>(upd)]
      ==
    ::
        %'send-message'
      ::  queue a message for a peer, bridge will poll /outbox
      =/  dest=@p
        =/  val  (~(got by obj) 'dest')
        ?>  ?=([%s *] val)
        (slav %p p.val)
      =/  payload=@t
        =/  val  (~(got by obj) 'payload')
        ?>  ?=([%s *] val)
        p.val
      =/  peer-entry  (~(get by peers) dest)
      ?~  peer-entry
        ~&  >>>  "lora-agent: unknown peer {<dest>}, register first"
        `this
      =/  msg=outbound-msg
        :*  next-msg-id
            dest
            dev-addr.u.peer-entry
            payload
            now.bowl
            %.n
        ==
      ~&  >  "lora-agent: queued message {<next-msg-id>} for {<dest>}"
      =.  outbox  (snoc outbox msg)
      =.  next-msg-id  +(next-msg-id)
      =/  upd=json
        %-  pairs:enjs:format
        :~  ['type' s+'message-queued']
            ['id' (numb:enjs:format id.msg)]
            ['dest' s+(scot %p dest)]
            ['payload' s+payload]
        ==
      :_  this
      :~  [%give %fact ~[/outbox] %json !>(upd)]
      ==
    ::
        %'message-received'
      ::  bridge pokes when inbound LoRa message arrives
      ::  resolve sender from peer map by DevAddr
      =/  src-addr=@t
        =/  val  (~(got by obj) 'src-addr')
        ?>  ?=([%s *] val)
        p.val
      =/  payload=@t
        =/  val  (~(got by obj) 'payload')
        ?>  ?=([%s *] val)
        p.val
      ::  look up sender ship by DevAddr
      =/  sender=(unit @p)
        =/  peer-list  ~(tap by peers)
        =/  found=(unit @p)  ~
        |-
        ?~  peer-list  found
        =/  item  i.peer-list
        ?:  =(dev-addr.q.item src-addr)
          (some p.item)
        $(peer-list t.peer-list)
      =/  sender-text=tape
        ?~  sender  "unknown"
        (trip (scot %p u.sender))
      ~&  >  "lora-agent: message from {sender-text} ({<src-addr>})"
      ::  update peer last-seen if known
      =?  peers  ?=(^ sender)
        =/  existing  (~(get by peers) u.sender)
        ?~  existing  peers
        (~(put by peers) u.sender u.existing(last-seen now.bowl, status %online))
      =/  msg=inbound-msg
        :*  next-msg-id
            sender
            src-addr
            payload
            now.bowl
        ==
      =.  inbox  (snoc inbox msg)
      =.  next-msg-id  +(next-msg-id)
      =/  upd=json
        %-  pairs:enjs:format
        :~  ['type' s+'new-message']
            ['id' (numb:enjs:format id.msg)]
            ['src-ship' ?~(sender s+'~' s+(scot %p u.sender))]
            ['src-addr' s+src-addr]
            ['payload' s+payload]
        ==
      :_  this
      :~  [%give %fact ~[/inbox] %json !>(upd)]
      ==
    ::
        %'tx-ack'
      ::  bridge confirms a message was transmitted
      =/  msg-id=@ud
        =/  val  (~(got by obj) 'msg-id')
        ?>  ?=([%n *] val)
        (rash p.val dem)
      ~&  >  "lora-agent: tx-ack for message {<msg-id>}"
      =.  outbox
        %+  turn  outbox
        |=  m=outbound-msg
        ?.  =(id.m msg-id)  m
        m(sent %.y)
      =/  upd=json
        %-  pairs:enjs:format
        :~  ['type' s+'message-sent']
            ['id' (numb:enjs:format msg-id)]
        ==
      :_  this
      :~  [%give %fact ~[/outbox] %json !>(upd)]
      ==
    ::
        %'tx-fail'
      ::  bridge reports transmission failure
      =/  msg-id=@ud
        =/  val  (~(got by obj) 'msg-id')
        ?>  ?=([%n *] val)
        (rash p.val dem)
      ~&  >  "lora-agent: tx-fail for message {<msg-id>}"
      ::  remove the failed message from outbox
      =.  outbox
        %+  skip  outbox
        |=(m=outbound-msg =(id.m msg-id))
      =/  upd=json
        %-  pairs:enjs:format
        :~  ['type' s+'message-failed']
            ['id' (numb:enjs:format msg-id)]
        ==
      :_  this
      :~  [%give %fact ~[/outbox] %json !>(upd)]
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
  ::
      [%peers ~]
    ~&  >  "lora-agent: subscriber on /peers"
    `this
  ::
      [%outbox ~]
    ~&  >  "lora-agent: subscriber on /outbox"
    `this
  ::
      [%inbox ~]
    ~&  >  "lora-agent: subscriber on /inbox"
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
  ::
  ::  === Existing scry endpoints ===
  ::
      [%x %stats ~]
    =/  result=json
      %-  pairs:enjs:format
      :~  ['device-count' (numb:enjs:format ~(wyt by devices))]
          ['uplink-count' (numb:enjs:format uplink-count)]
          ['peer-count' (numb:enjs:format ~(wyt by peers))]
          ['outbox-count' (numb:enjs:format (lent outbox))]
          ['inbox-count' (numb:enjs:format (lent inbox))]
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
  ::
  ::  === Peer-to-peer scry endpoints (Phase 3c) ===
  ::
      [%x %peers ~]
    =/  peer-list=(list [@p peer])  ~(tap by peers)
    =/  result=json
      :-  %a
      %+  turn  peer-list
      |=  [key=@p p=peer]
      %-  pairs:enjs:format
      :~  ['ship' s+(scot %p ship.p)]
          ['dev-addr' s+dev-addr.p]
          ['last-seen' (sect:enjs:format last-seen.p)]
          ['status' s+?-(status.p %online 'online', %offline 'offline')]
      ==
    ``json+!>(result)
  ::
      [%x %outbox ~]
    =/  pending=(list outbound-msg)
      (skip outbox |=(m=outbound-msg sent.m))
    =/  result=json
      :-  %a
      %+  turn  pending
      |=  m=outbound-msg
      %-  pairs:enjs:format
      :~  ['id' (numb:enjs:format id.m)]
          ['dest-ship' s+(scot %p dest-ship.m)]
          ['dest-addr' s+dest-addr.m]
          ['src-addr' ?~(my-addr s+'' s+u.my-addr)]
          ['payload' s+payload.m]
          ['queued-at' (sect:enjs:format queued-at.m)]
      ==
    ``json+!>(result)
  ::
      [%x %inbox ~]
    =/  result=json
      :-  %a
      %+  turn  inbox
      |=  m=inbound-msg
      %-  pairs:enjs:format
      :~  ['id' (numb:enjs:format id.m)]
          ['src-ship' ?~(src-ship.m s+'~' s+(scot %p u.src-ship.m))]
          ['src-addr' s+src-addr.m]
          ['payload' s+payload.m]
          ['received-at' (sect:enjs:format received-at.m)]
      ==
    ``json+!>(result)
  ==
::
++  on-agent
  |=  [=wire =sign:agent:gall]
  ^-  (quip card _this)
  (on-agent:def wire sign)
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
