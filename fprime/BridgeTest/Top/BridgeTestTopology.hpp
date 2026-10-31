/*
 * BridgeTestTopology.hpp -- setting BridgeTest up, running it and taking it
 * down, for main().
 */

#ifndef BRIDGETEST_BRIDGETESTTOPOLOGY_HPP
#define BRIDGETEST_BRIDGETESTTOPOLOGY_HPP

#include <BridgeTest/Top/BridgeTestTopologyDefs.hpp>

namespace BridgeTest {

/** Sets the topology up: every component made, wired and configured, the
 * bridge opened on the configuration, and every task started.
 *
 * @param state  What main() was told: where F´'s ground system is and the
 *               configuration the bridge opens. */
void setupTopology(const TopologyState &state);

/** Runs the rate groups, until stopRateGroups() is called. */
void startRateGroups();

/** Stops the rate groups, from a signal handler. */
void stopRateGroups();

/** Takes the topology down, its tasks stopped and joined.
 *
 * @param state  What setupTopology() was given. */
void teardownTopology(const TopologyState &state);

} // namespace BridgeTest

#endif
