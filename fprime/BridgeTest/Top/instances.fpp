# The instances of BridgeTest: the bridge, and the least of an F´ deployment
# that lets F´'s own ground system command it and read its channels.
#
# Made from F´'s deployment template and its Ref deployment, with what this
# deployment does not use taken out: no sequencers, no data products, no
# demonstration components.

module BridgeTest {

  module Default {
    constant QUEUE_SIZE = 10
    constant STACK_SIZE = 64 * 1024
  }

  # The bridge's thread runs tcslib-c, which is Rust, and needs the stack a
  # Rust thread is given rather than F´'s usual.
  constant BRIDGE_STACK_SIZE = 2 * 1024 * 1024

  # --------------------------------------------------------------------
  # The bridge
  # --------------------------------------------------------------------

  # Its base ID is the gateway node's fprime_id_base, which the model says:
  # the ID of each channel within the component is then the device id of
  # the node it carries, as docs/design.rst has it, and the bridge refuses
  # to run under any other.
  instance tcsBridge: Tcs.TcsBridge base id Tcs.TcsBridge.ID_BASE \
    queue size Default.QUEUE_SIZE \
    stack size BRIDGE_STACK_SIZE \
    priority 30

  # --------------------------------------------------------------------
  # Active component instances
  # --------------------------------------------------------------------

  instance rateGroup1Comp: Svc.ActiveRateGroup base id 0x10001000 \
    queue size Default.QUEUE_SIZE \
    stack size Default.STACK_SIZE \
    priority 43

  instance rateGroup2Comp: Svc.ActiveRateGroup base id 0x10002000 \
    queue size Default.QUEUE_SIZE \
    stack size Default.STACK_SIZE \
    priority 42

  # --------------------------------------------------------------------
  # Passive component instances
  # --------------------------------------------------------------------

  instance posixTime: Svc.PosixTime base id 0x10020000

  instance rateGroupDriverComp: Svc.RateGroupDriver base id 0x10021000

  instance linuxTimer: Svc.LinuxTimer base id 0x10024000

  instance comDriver: Drv.TcpClient base id 0x10025000

}
