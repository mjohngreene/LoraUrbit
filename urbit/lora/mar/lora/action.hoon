::  mar/lora/action.hoon — Mark for %lora-agent poke actions
::
::  Converts JSON from the Rust bridge into +action nouns.
::  The bridge sends JSON like:
::    {"action": "uplink", "dev-addr": "01ab...", ...}
::
/-  *lora-agent
|_  act=action
++  grab
  |%
  ++  noun  action
  ++  json
    |=  jon=json
    ^-  action
    =<  (parse-action jon)
    |%
    ++  parse-action
      |=  jon=json
      ^-  action
      =/  obj  ((om:dejs:format jon) ~)
      =/  typ  (so:dejs:format (~(got by obj) 'action'))
      ?+  typ  ~|("unknown lora action: {<typ>}" !!)
        %'uplink'           [%uplink (parse-uplink jon)]
        %'register-device'  (parse-register jon)
        %'downlink-request' (parse-downlink-req jon)
        %'downlink-ack'     (parse-downlink-ack jon)
      ==
    ::
    ++  parse-uplink
      |=  jon=json
      ^-  uplink
      %.  jon
      %-  ot:dejs:format
      :~  ['dev-addr' so:dejs:format]
          ['fcnt' ni:dejs:format]
          ['f-port' (mu:dejs:format ni:dejs:format)]
          ['payload' so:dejs:format]
          ['rssi' (su:dejs:format ;~(pose dem ;~(plug hep dem)))]
          ['snr' (mu:dejs:format (su:dejs:format ;~(pose dem ;~(plug hep dem))))]
          ['freq' so:dejs:format]
          ['data-rate' so:dejs:format]
          ['gateway-eui' so:dejs:format]
          ['received-at' di:dejs:format]
          ['mtype' (cu:dejs:format parse-mtype so:dejs:format)]
          ['source' (cu:dejs:format parse-source so:dejs:format)]
      ==
    ::
    ++  parse-register
      |=  jon=json
      ^-  action
      =/  r  %.  jon
        %-  ot:dejs:format
        :~  ['dev-addr' so:dejs:format]
            ['name' (mu:dejs:format so:dejs:format)]
            ['description' (mu:dejs:format so:dejs:format)]
        ==
      [%register-device r]
    ::
    ++  parse-downlink-req
      |=  jon=json
      ^-  action
      =/  r  %.  jon
        %-  ot:dejs:format
        :~  ['dev-addr' so:dejs:format]
            ['f-port' ni:dejs:format]
            ['payload' so:dejs:format]
            ['confirmed' bo:dejs:format]
        ==
      [%downlink-request r]
    ::
    ++  parse-downlink-ack
      |=  jon=json
      ^-  action
      =/  r  %.  jon
        %-  ot:dejs:format
        :~  ['dev-addr' so:dejs:format]
            ['success' bo:dejs:format]
        ==
      [%downlink-ack r]
    ::
    ++  parse-mtype
      |=  t=@t
      ^-  mtype
      ?+  t  ~|("unknown mtype: {<t>}" !!)
        %'join-request'           %join-request
        %'join-accept'            %join-accept
        %'unconfirmed-data-up'    %unconfirmed-data-up
        %'unconfirmed-data-down'  %unconfirmed-data-down
        %'confirmed-data-up'      %confirmed-data-up
        %'confirmed-data-down'    %confirmed-data-down
        %'rejoin-request'         %rejoin-request
        %'proprietary'            %proprietary
      ==
    ::
    ++  parse-source
      |=  t=@t
      ^-  packet-source
      ?+  t  ~|("unknown source: {<t>}" !!)
        %'local'   %local
        %'helium'  %helium
      ==
    --
  --
++  grow
  |%
  ++  noun  act
  --
++  grad  %noun
--
