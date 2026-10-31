# How BridgeTest is wired: F´'s command and data handling and its CCSDS
# communications, as the Ref deployment has them, and the bridge on the
# first rate group.

module BridgeTest {

  enum Ports_RateGroups {
    rateGroup1
    rateGroup2
  }

  deployment topology BridgeTest {

    # ------------------------------------------------------------------
    # Subtopology instances
    # ------------------------------------------------------------------

    instance CdhCore.Subtopology
    instance ComCcsds.Subtopology
    instance FileHandling.Subtopology

    # ------------------------------------------------------------------
    # Instances used in the topology
    # ------------------------------------------------------------------

    instance tcsBridge
    instance posixTime
    instance rateGroup1Comp
    instance rateGroup2Comp
    instance rateGroupDriverComp
    instance linuxTimer
    instance comDriver

    # ------------------------------------------------------------------
    # Pattern graph specifiers
    # ------------------------------------------------------------------

    command connections instance CdhCore.cmdDisp

    event connections instance CdhCore.events

    telemetry connections instance CdhCore.tlmSend

    text event connections instance CdhCore.textLogger

    health connections instance CdhCore.$health

    param connections instance FileHandling.prmDb

    time connections instance posixTime

    # ------------------------------------------------------------------
    # Direct graph specifiers
    # ------------------------------------------------------------------

    connections RateGroups {

      linuxTimer.CycleOut -> rateGroupDriverComp.CycleIn

      # Rate group 1, at the base clock's rate. The bridge takes its data
      # here, so a channel is at most a tick old when it is sent.
      rateGroupDriverComp.CycleOut[Ports_RateGroups.rateGroup1] \
        -> rateGroup1Comp.CycleIn
      rateGroup1Comp.RateGroupMemberOut[0] -> CdhCore.Subtopology.tlmSendRun
      rateGroup1Comp.RateGroupMemberOut[1] \
        -> FileHandling.Subtopology.fileDownlinkRun
      rateGroup1Comp.RateGroupMemberOut[2] -> ComCcsds.Subtopology.comQueueRun
      rateGroup1Comp.RateGroupMemberOut[3] -> CdhCore.Subtopology.cmdDispRun
      rateGroup1Comp.RateGroupMemberOut[4] \
        -> ComCcsds.Subtopology.aggregatorTimeout
      rateGroup1Comp.RateGroupMemberOut[5] -> tcsBridge.schedIn

      # Rate group 2, at a quarter of it.
      rateGroupDriverComp.CycleOut[Ports_RateGroups.rateGroup2] \
        -> rateGroup2Comp.CycleIn
      rateGroup2Comp.RateGroupMemberOut[0] -> CdhCore.Subtopology.healthRun
      rateGroup2Comp.RateGroupMemberOut[1] \
        -> ComCcsds.Subtopology.bufferManagerSchedIn
      rateGroup2Comp.RateGroupMemberOut[2] -> CdhCore.Subtopology.eventsRun
    }

    connections Communications {
      comDriver.allocate   -> ComCcsds.Subtopology.commsBufferGetCallee
      comDriver.deallocate -> ComCcsds.Subtopology.commsBufferSendIn

      comDriver.$recv \
        -> ComCcsds.Subtopology.drvReceiveIn
      ComCcsds.Subtopology.drvReceiveReturnOut -> comDriver.recvReturnIn

      ComCcsds.Subtopology.drvSendOut -> comDriver.$send
      comDriver.ready                 -> ComCcsds.Subtopology.drvConnected
    }

    connections ComCcsds_CdhCore {
      CdhCore.Subtopology.eventsPktSend \
        -> ComCcsds.Subtopology.comPacketQueueIn[ \
          ComCcsds.Ports_ComPacketQueue.EVENTS]
      CdhCore.Subtopology.tlmSendPktSend \
        -> ComCcsds.Subtopology.comPacketQueueIn[ \
          ComCcsds.Ports_ComPacketQueue.TELEMETRY]

      ComCcsds.Subtopology.commandOut  -> CdhCore.Subtopology.seqCmdBuff
      CdhCore.Subtopology.seqCmdStatus -> ComCcsds.Subtopology.cmdResponseIn
    }

    connections ComCcsds_FileHandling {
      FileHandling.Subtopology.fileDownlinkBufferSendOut \
        -> ComCcsds.Subtopology.bufferQueueIn[ \
          ComCcsds.Ports_ComBufferQueue.FILE]
      ComCcsds.Subtopology.bufferReturnOut[ \
          ComCcsds.Ports_ComBufferQueue.FILE] \
        -> FileHandling.Subtopology.fileDownlinkBufferReturn

      ComCcsds.Subtopology.fileUplinkOut \
        -> FileHandling.Subtopology.fileUplinkBufferSendIn
      FileHandling.Subtopology.fileUplinkBufferSendOut \
        -> ComCcsds.Subtopology.fileUplinkReturnIn
    }

  }

}
