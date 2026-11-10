# TcsBridge: tcspecial's commander, inside an F´ deployment.
#
# To tcspecial the bridge is a ground station: it opens the configuration
# the vehicle flies, sends CONNECT and compares checksums, takes beacons,
# sends commands and data and receives data, all over the radio's UDP
# protocol, through tcslib-c's tcs.h. To the rest of the deployment it is a component
# like any other: commands in, events and telemetry channels out.
#
# Which channels it has, and their IDs, are the configuration's: the model
# included at the end is written from it by
#     tcsverify --fprime-model=fprime/Tcs/TcsBridge/model <configuration>
# and never by hand. See "F´: gateway_fprime" in docs/design.rst.

module Tcs {

  @ How many bytes of one datum a channel holds. A longer datum's first
  @ DATUM_BYTES go, with its whole length, and it is counted in DataCutShort
  constant DATUM_BYTES = 64

  @ The bytes of a datum, as many as a channel holds
  array DatumBytes = [DATUM_BYTES] U8

  @ One datum from a node drawn into the gateway node, as a channel holds it
  struct Datum {
    @ How many bytes the node produced, which may be more than are here
    length: U32
    @ Its first bytes, as many as there are or DATUM_BYTES
    bytes: DatumBytes
  }

  @ How long it has been since the vehicle's last beacon, counted in
  @ beacons missed in a row
  enum Stage : U8 {
    @ Nothing has arrived
    UNHEARD = 0
    @ Fewer than four missed
    HEARD = 1
    @ Four to seven missed
    AGING = 2
    @ Eight or more: waiting to be heard from again
    WAITING = 3
  }

  @ The bridge between an F´ deployment and tcspecial
  active component TcsBridge {

    # ------------------------------------------------------------------
    # Ports
    # ------------------------------------------------------------------

    @ The tick on which data is taken, channels are written and, while
    @ the bridge is not matched, a CONNECT is sent
    async input port schedIn: Svc.Sched drop

    @ Time get port
    time get port timeCaller

    import Fw.Command
    import Fw.Event
    import Fw.Channel

    # ------------------------------------------------------------------
    # Commands: one for each tcspecial command the bridge sends, and one
    # for data up to the gateway node
    # ------------------------------------------------------------------

    @ Ask whether tcspecial's command link answers
    async command PING opcode 0

    @ Start a node
    async command START_NODE(
      node: string size NODE_NAME_BYTES @< The node's name
    ) opcode 1

    @ Stop a node
    async command STOP_NODE(
      node: string size NODE_NAME_BYTES @< The node's name
    ) opcode 2

    @ Ask whether a node is running
    async command QUERY_NODE(
      node: string size NODE_NAME_BYTES @< The node's name
    ) opcode 3

    @ Arm a restart of the vehicle onto a configuration it holds, which
    @ the RESTART that follows must name with the same key
    async command RESTART_ARM(
      key: U64 @< The key, which the RESTART must carry
      $target: string size 32 @< The configuration's checksum, as tcsverify prints it, or empty for this one's
    ) opcode 5

    @ Restart the vehicle onto the configuration the last RESTART_ARM
    @ named. Its beacons say whether it was taken; the bridge connects to
    @ the start that comes back by itself
    async command RESTART(
      key: U64 @< The key the last RESTART_ARM carried
      $target: string size 32 @< The checksum it carried, or empty for this configuration's
    ) opcode 6

    @ Send a datum up to the gateway node, which leaves it by its
    @ moc-out links for the nodes they reach: 1 or 0 to a heater, say.
    @ Sent only while the bridge is matched for commanding
    async command SEND_DATA(
      data: string size DATUM_BYTES @< The datum, as text
    ) opcode 4

    # ------------------------------------------------------------------
    # Events
    # ------------------------------------------------------------------

    @ The configuration could not be opened, so the bridge does nothing
    event OpenFailed(
      why: string size 256 @< What tcslib-c said
    ) severity warning high id 0 \
      format "The bridge does nothing: {}"

    @ The configuration opened is not the one the model was written from:
    @ write the model again with tcsverify --fprime-model and rebuild, or
    @ open the configuration it was written from
    event ModelDiffers(
      opened: string size 40 @< The digest of the configuration opened
      model: string size 40 @< The digest the model was written from
    ) severity warning high id 1 \
      format "Opened md5 {}, model of md5 {}: run tcsverify --fprime-model"

    @ The base ID the deployment gave the bridge is not the gateway node's
    @ fprime_id_base: give the instance base id Tcs.TcsBridge.ID_BASE
    event BaseDiffers(
      given: U32 @< The instance's base ID
      model: U32 @< fprime_id_base
    ) severity warning high id 2 \
      format "Base ID {x} is not fprime_id_base {x}: use Tcs.TcsBridge.ID_BASE"

    @ The beacon watch failed, and is tried again
    event BeaconsFailed(
      why: string size 256 @< What tcslib-c said
    ) severity warning low id 3 \
      format "Beacons: {}" \
      throttle 10

    @ What the vehicle's beacons say has changed. A channel's text is
    @ shorter than a beacon's, so it is said here, each time it changes
    event BeaconSaid(
      said: string size 128 @< What the last beacon said
    ) severity activity high id 15 \
      format "The vehicle's beacon says: {}"

    @ A CONNECT was answered with this configuration's checksum
    event Matched severity activity high id 4 \
      format "Connected: tcspecial is flying this configuration"

    @ A CONNECT was answered with another configuration's checksum.
    @ Nothing but CONNECT goes until the two agree
    event ConfigurationDiffers(
      said: string size 128 @< What the vehicle's last beacon said
    ) severity warning high id 5 \
      format "tcspecial flies another configuration: its beacon says {}"

    @ A CONNECT could not be sent or was not answered
    event ConnectFailed(
      why: string size 256 @< What tcslib-c said
    ) severity warning low id 6 \
      format "CONNECT: {}" \
      throttle 10

    @ A PING was answered
    event PingAnswered severity activity high id 7 \
      format "PING answered"

    @ A command was not sent, or not answered
    event CommandFailed(
      cmd: string size 16 @< The command
      why: string size 256 @< What tcslib-c said
    ) severity warning low id 8 \
      format "{}: {}"

    @ A node command named a node the configuration does not have
    event NoSuchNode(
      cmd: string size 16 @< The command
      node: string size NODE_NAME_BYTES @< The name given
    ) severity warning low id 9 \
      format "{}: the configuration has no node called {}"

    @ tcspecial answered a node command
    event NodeAnswered(
      cmd: string size 16 @< The command
      node: string size NODE_NAME_BYTES @< The node
      status: string size 32 @< What tcspecial answered
    ) severity activity high id 10 \
      format "{} {}: {}"

    @ tcspecial said whether a node is running
    event NodeQueried(
      node: string size NODE_NAME_BYTES @< The node
      running: bool @< Whether it is running
    ) severity activity high id 11 \
      format "{} is running: {}"

    @ A datum was longer than a channel holds: its first DATUM_BYTES went,
    @ with its whole length
    event DatumCutShort(
      source: string size NODE_NAME_BYTES @< The node that produced it
      length: U32 @< How long it was
    ) severity warning low id 12 \
      format "{} sent {} bytes: a channel holds the first of them" \
      throttle 10

    @ A datum came from a source the model has no channel for, which a
    @ model written from this configuration never lacks: write the model
    @ again with tcsverify --fprime-model and rebuild
    event NoChannel(
      source: string size NODE_NAME_BYTES @< The node that produced it
    ) severity warning high id 13 \
      format "{} sent a datum the model has no channel for: rewrite it" \
      throttle 10

    @ A node the configuration says sends values sent a datum that is not
    @ one: it went to no channel, and is counted in DataNotValues
    event NotAValue(
      source: string size NODE_NAME_BYTES @< The node that sent it
      why: string size 256 @< What tcslib-c said
    ) severity warning low id 16 \
      format "{} sent a datum that is not a value: {}" \
      throttle 10

    @ A node the configuration says sends decisions sent a datum that is
    @ not one: it went to no channel, and is counted in DataNotDecisions
    event NotADecision(
      source: string size NODE_NAME_BYTES @< The node that sent it
      why: string size 256 @< What tcslib-c said
    ) severity warning low id 19 \
      format "{} sent a datum that is not a decision: {}" \
      throttle 10

    @ A node the configuration says sends a whole number sent a datum that
    @ is not one of its type's width: it went to no channel, and is counted
    @ in DataNotWholes
    event NotAWhole(
      source: string size NODE_NAME_BYTES @< The node that sent it
      why: string size 256 @< What tcslib-c said
    ) severity warning low id 20 \
      format "{} sent a datum that is not a whole number of its type: {}" \
      throttle 10

    @ tcspecial answered RESTART_ARM or RESTART
    event RestartAnswered(
      cmd: string size 16 @< The command
      status: string size 32 @< What tcspecial answered
    ) severity activity high id 18 \
      format "{}: {}"

    @ A datum went up to the gateway node
    event DataSent(
      length: U32 @< How many bytes it was
    ) severity activity low id 17 \
      format "{} bytes sent up to the gateway node"

    @ Taking data failed
    event ReceiveFailed(
      why: string size 256 @< What tcslib-c said
    ) severity warning low id 14 \
      format "Data: {}" \
      throttle 10

    # ------------------------------------------------------------------
    # The model: constants and every channel, written from the
    # configuration
    # ------------------------------------------------------------------

    include "model/TcsBridgeModel.fppi"

  }

}
