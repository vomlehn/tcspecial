/*
 * BridgeTestTopologyDefs.hpp -- what F´'s topology autocoder requires of
 * BridgeTest: the state carried into setting it up, and how many health
 * pings each active component may miss.
 */

#ifndef BRIDGETEST_BRIDGETESTTOPOLOGYDEFS_HPP
#define BRIDGETEST_BRIDGETESTTOPOLOGYDEFS_HPP

#include "BridgeTest/Top/FppConstantsAc.hpp"
#include "Svc/Subtopologies/CdhCore/PingEntries.hpp"
#include "Svc/Subtopologies/CdhCore/SubtopologyTopologyDefs.hpp"
#include "Svc/Subtopologies/ComCcsds/PingEntries.hpp"
#include "Svc/Subtopologies/ComCcsds/Ports_ComBufferQueueEnumAc.hpp"
#include "Svc/Subtopologies/ComCcsds/Ports_ComPacketQueueEnumAc.hpp"
#include "Svc/Subtopologies/ComCcsds/SubtopologyTopologyDefs.hpp"
#include "Svc/Subtopologies/FileHandling/PingEntries.hpp"
#include "Svc/Subtopologies/FileHandling/SubtopologyTopologyDefs.hpp"

/* Missed health pings allowed before a warning, and before F´ gives up on
 * the component, for each active component that answers them. The bridge
 * answers none: a command to the vehicle may hold its thread for as long as
 * tcslib-moc-c waits for an answer, which is not a hung thread. */
namespace PingEntries {
namespace BridgeTest_rateGroup1Comp {
enum { WARN = 3, FATAL = 5 };
}
namespace BridgeTest_rateGroup2Comp {
enum { WARN = 3, FATAL = 5 };
}
} // namespace PingEntries

namespace BridgeTest {

/* What main() hands the topology to set it up with. */
struct TopologyState {
	const char *hostname;      /* where F´'s ground system is */
	U16 port;                  /* and its port */
	const char *configuration; /* the tcspecial configuration */
	CdhCore::SubtopologyState cdhCore;
	ComCcsds::SubtopologyState comCcsds;
	FileHandling::SubtopologyState fileHandling;
};

namespace PingEntries = ::PingEntries;

} // namespace BridgeTest

#endif
