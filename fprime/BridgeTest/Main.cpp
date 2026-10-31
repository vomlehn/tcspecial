/*
 * Main.cpp -- BridgeTest, the F´ deployment the TcsBridge component is
 * tested in.
 *
 *     BridgeTest -c <configuration> [-a <address> -p <port>]
 *
 * -c names the tcspecial configuration the bridge opens, which must be the
 * one its model was written from. -a and -p say where F´'s ground system
 * listens, which fprime-gds passes itself when it starts the deployment.
 * The rate groups run until SIGINT or SIGTERM.
 */

#include <csignal>
#include <cstdlib>
#include <getopt.h>

#include <BridgeTest/Top/BridgeTestTopology.hpp>
#include <Fw/Logger/Logger.hpp>
#include <Os/Os.hpp>

namespace {

/** Says how the program is run.
 *
 * @param program  The program's name, as it was run. */
void usage(const char *program)
{
	Fw::Logger::log("usage: %s -c <configuration> [-a <address> "
			"-p <port>]\n",
			program);
}

/** Stops the rate groups, so that main() goes on to take the topology
 * down.
 *
 * @param signal  The signal, which is either. */
void stop(int signal)
{
	(void)signal;
	BridgeTest::stopRateGroups();
}

} // namespace

/** Sets the topology up, runs it until stopped, and takes it down.
 *
 * @param argc  How many arguments there are, the program's name included.
 * @param argv  The arguments, as the comment at the top of this file says.
 * @return      0 when it ran and stopped, 1 when the arguments are
 *              wrong. */
int main(int argc, char *argv[])
{
	BridgeTest::TopologyState state{};
	int option = 0;

	Os::init();
	while ((option = getopt(argc, argv, "ha:p:c:")) != -1) {
		switch (option) {
		case 'a':
			state.hostname = optarg;
			break;
		case 'p':
			state.port = static_cast<U16>(std::atoi(optarg));
			break;
		case 'c':
			state.configuration = optarg;
			break;
		case 'h':
			usage(argv[0]);
			return 0;
		default:
			usage(argv[0]);
			return 1;
		}
	}
	// Without a configuration the bridge has nothing to open, and would
	// run saying so for as long as it ran.
	if (state.configuration == nullptr) {
		Fw::Logger::log("%s: -c names the tcspecial configuration the "
				"bridge opens, and none was given\n",
				argv[0]);
		usage(argv[0]);
		return 1;
	}

	std::signal(SIGINT, stop);
	std::signal(SIGTERM, stop);
	BridgeTest::setupTopology(state);
	BridgeTest::startRateGroups();
	BridgeTest::teardownTopology(state);
	return 0;
}
