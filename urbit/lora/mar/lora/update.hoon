::  mar/lora/update.hoon — Mark for %lora-agent subscription updates
::
::  Converts +update nouns into JSON for subscribers.
::
/-  *lora-agent
|_  upd=update
++  grab
  |%
  ++  noun  update
  --
++  grow
  |%
  ++  noun  upd
  ++  json
    =,  enjs:format
    ^-  ^json
    ?-  -.upd
      %new-uplink
        %-  pairs
        :~  ['type' s+'new-uplink']
            ['uplink' (uplink-to-json uplink.upd)]
        ==
    ::
      %device-update
        %-  pairs
        :~  ['type' s+'device-update']
            ['device' (device-to-json device.upd)]
        ==
    ::
      %downlink-sent
        %-  pairs
        :~  ['type' s+'downlink-sent']
            ['dev-addr' s+dev-addr.upd]
            ['success' b+success.upd]
        ==
    ::
      %initial-devices
        %-  pairs
        :~  ['type' s+'initial-devices']
            ['devices' a+(turn devices.upd device-to-json)]
        ==
    ==
  --
++  grad  %noun
::
++  uplink-to-json
  |=  u=uplink
  =,  enjs:format
  ^-  json
  %-  pairs
  :~  ['dev-addr' s+dev-addr.u]
      ['fcnt' (numb fcnt.u)]
      ['f-port' ?~(f-port.u ~ (numb u.f-port.u))]
      ['payload' s+payload.u]
      ['freq' s+freq.u]
      ['data-rate' s+data-rate.u]
      ['gateway-eui' s+gateway-eui.u]
      ['received-at' (sect received-at.u)]
      ['mtype' s+(mtype-to-cord mtype.u)]
      ['source' s+(source-to-cord source.u)]
  ==
::
++  device-to-json
  |=  d=device
  =,  enjs:format
  ^-  json
  %-  pairs
  :~  ['dev-addr' s+dev-addr.d]
      ['name' ?~(name.d ~ s+u.name.d)]
      ['description' ?~(description.d ~ s+u.description.d)]
      ['last-seen' (sect last-seen.d)]
      ['packet-count' (numb packet-count.d)]
  ==
::
++  mtype-to-cord
  |=  m=mtype
  ^-  @t
  ?-  m
    %join-request           'join-request'
    %join-accept            'join-accept'
    %unconfirmed-data-up    'unconfirmed-data-up'
    %unconfirmed-data-down  'unconfirmed-data-down'
    %confirmed-data-up      'confirmed-data-up'
    %confirmed-data-down    'confirmed-data-down'
    %rejoin-request         'rejoin-request'
    %proprietary            'proprietary'
  ==
::
++  source-to-cord
  |=  s=packet-source
  ^-  @t
  ?-  s
    %local   'local'
    %helium  'helium'
  ==
--
