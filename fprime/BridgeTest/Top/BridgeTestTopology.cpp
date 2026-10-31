/*
 * BridgeTestTopology.cpp -- setting BridgeTest up and taking it down.
 *
 * In the order F´'s Ref deployment has it, which the autocoded functions
 * need: made, given base IDs, wired, configured, parameters read, tasks
 * started. The bridge is opened on its configuration while being
 * configured, so that the beacon watch and the data are there before
 * anything is asked of them.
 */

#include <BridgeTest/Top/BridgeTestTopology.hpp>
#include <BridgeTest/Top/BridgeTestTopologyAc.hpp>

namespace BridgeTest {

namespace {

/* The base clock, and how often each rate group runs on it: the first on
 * every tick, the second on every fourth. */
const Fw::TimeInterval RATE_GROUP_INTERVAL(1, 0);
Svc::RateGroupDriver::DividerSet rateGroupDivisors{{{1, 0}, {4, 0}}};

/* What each rate group hands its members, which none of them reads. */
Svc::ActiveRateGroup::ContextArray rateGroup1Context(0);
Svc::ActiveRateGroup::ContextArray rateGroup2Context(0);

/* The priority of the task that reads what F´'s ground system sends. */
const FwTaskPriorityType COMM_PRIORITY = 34;

/** Configures what the autocoder leaves to the deployment: the rate groups'
 * divisors and contexts, where files go, and the bridge.
 *
 * @param state  What main() was told. */
void configureTopology(const TopologyState &state)
{
	rateGroupDriverComp.configure(rateGroupDivisors);
	rateGroup1Comp.configure(rateGroup1Context);
	rateGroup2Comp.configure(rateGroup2Context);
	FileHandling::fileUplink.configure(".");
	FileHandling::fileDownlink.configure(".");
	FileHandling::prmDb.configureSandbox(".");
	tcsBridge.configure(state.configuration);
}

} // namespace

void setupTopology(const TopologyState &state)
{
	initComponents(state);
	setBaseIds();
	connectComponents();
	configComponents(state);
	if (state.hostname != nullptr && state.port != 0) {
		comDriver.configure(state.hostname, state.port);
	}
	configureTopology(state);
	regCommands();
	readParameters();
	loadParameters();
	startTasks(state);
	if (state.hostname != nullptr && state.port != 0) {
		Os::TaskString name("ReceiveTask");
		comDriver.start(name, COMM_PRIORITY, Default::STACK_SIZE);
	}
}

void startRateGroups()
{
	linuxTimer.startTimer(RATE_GROUP_INTERVAL);
}

void stopRateGroups()
{
	linuxTimer.quit();
}

void teardownTopology(const TopologyState &state)
{
	stopTasks(state);
	freeThreads(state);
	comDriver.stop();
	(void)comDriver.join();
	tcsBridge.stop();
	tearDownComponents(state);
	deinitComponents(state);
}

} // namespace BridgeTest
