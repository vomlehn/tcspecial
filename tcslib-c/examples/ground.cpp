// A ground station in C++, on tcs.h.
//
// The same program as ground.c, written as C++ is: the header included as it
// stands, each handle held by a std::unique_ptr that closes it, and strings
// as std::string. It prints the same lines, which is what the test holds the
// two to.
//
// Arguments, all optional: --yaml or --xml, but not both, saying which of the
// example's two configuration files to read (YAML where neither is given);
// --restart, to arm a restart of the vehicle and restart it, which a station
// does only when it means to; the configuration; the gateway node to take data
// from; and the node to ask about. Run from the repository's root none need
// be given, and it watches the ping example.
//
//     c++ ground.cpp -Iinclude target/debug/libtcs.a -lpthread -ldl -lm

#include <cstdint>
#include <cstdio>
#include <cstring>
#include <iostream>
#include <memory>
#include <string>
#include <vector>

#include "tcs.h"

namespace {

// The example's configuration, without its extension.
const std::string STEM = "examples/ping/ping";

// The key a restart is armed with, which the restart must carry.
const std::uint64_t ARM_KEY = 0x1234;

/** Says whether the station is matched with the vehicle, for listening and
 * for commanding.
 *
 * @param configuration  The configuration the station uses.
 * @return               true, or false where it cannot be asked. */
bool say_matched(const tcs_configuration *configuration)
{
	int listening = 0, commanding = 0;
	if (tcs_configuration_matched(configuration, &listening,
				      &commanding) != 0) {
		return false;
	}
	std::printf("matched: listening %s, commanding %s\n",
		    listening ? "yes" : "no", commanding ? "yes" : "no");
	return true;
}

// Each handle, closed by its own _close when it goes.
template <typename T, void (*Close)(T *)> struct Closer {
	void operator()(T *handle) const { Close(handle); }
};
using Configuration = std::unique_ptr<
	tcs_configuration,
	Closer<tcs_configuration, tcs_configuration_close>>;
using BeaconWatch =
	std::unique_ptr<tcs_beacons, Closer<tcs_beacons, tcs_beacons_close>>;
using GatewayNode =
	std::unique_ptr<tcs_gateway, Closer<tcs_gateway, tcs_gateway_close>>;
using Client =
	std::unique_ptr<tcs_client, Closer<tcs_client, tcs_client_close>>;
using PingerHandle =
	std::unique_ptr<tcs_pinger, Closer<tcs_pinger, tcs_pinger_close>>;

// Say what failed, and stop.
int fail(const std::string &what)
{
	std::cerr << what << ": " << tcs_last_error() << '\n';
	return 1;
}

/** Says what a status means here, by the header's own constants, so that a
 * header whose numbers had drifted from the library's would say so: done,
 * or refused and the status's name, or its number for one this library does
 * not know -- a vehicle of a later build may answer with one, and it is a
 * refusal all the same.
 *
 * @param status  The status the vehicle answered with.
 * @return        The words. */
std::string outcome(tcs_status status)
{
	const std::string name = tcs_status_name(status);
	if (status == TCS_STATUS_SUCCESS) {
		return "done";
	}
	if (!name.empty()) {
		return "refused (" + name + ")";
	}
	return "refused (status " + std::to_string(static_cast<int>(status)) +
	       ")";
}

} // namespace

/** Runs the ground station's steps in order, printing a line for each.
 *
 * @param argc  How many arguments there are, the program's name included.
 * @param argv  The arguments, as the comment at the top of this file says.
 * @return      0 when every step was done, 1 when one failed (after saying
 *              why on standard error), 2 when the arguments are wrong. */
int main(int argc, char **argv)
{
	std::string spelling;
	std::vector<std::string> named;
	bool restart = false;

	for (int i = 1; i < argc; i++) {
		const std::string arg = argv[i];
		if (arg == "--yaml" || arg == "--xml") {
			const std::string asked = arg.substr(2);
			if (!spelling.empty() && spelling != asked) {
				std::cerr << "--yaml and --xml each say which "
					     "configuration to read: give one "
					     "of them, or neither\n";
				return 2;
			}
			spelling = asked;
		} else if (arg == "--restart") {
			restart = true;
		} else if (!arg.empty() && arg[0] == '-') {
			std::cerr << arg
				  << " is not an option: the options are "
				     "--yaml, --xml and --restart\n";
			return 2;
		} else if (named.size() == 3) {
			std::cerr << arg
				  << ": a configuration, a gateway node and a "
				     "node may be named, and nothing more\n";
			return 2;
		} else {
			named.push_back(arg);
		}
	}
	const std::string path = named.size() > 0 ? named[0] :
		STEM + "." + (spelling.empty() ? "yaml" : spelling);
	const std::string gateway_name =
		named.size() > 1 ? named[1] : "gateway";
	const std::string node_name = named.size() > 2 ? named[2] : "ping";

	Configuration configuration(tcs_configuration_open(path.c_str()));
	if (!configuration) {
		return fail("configuration");
	}
	std::printf("configuration: md5 %s\n",
		    tcs_configuration_digest(configuration.get()));

	// Bound before anything is asked, so that nothing the vehicle sends in
	// answer arrives before there is anywhere for it to go.
	BeaconWatch beacons(tcs_beacons_listen(configuration.get()));
	GatewayNode gateway(
		tcs_gateway_open(configuration.get(), gateway_name.c_str()));
	Client client(tcs_client_open(configuration.get()));
	PingerHandle pinger(tcs_pinger_open(configuration.get()));
	if (!beacons || !gateway || !client || !pinger) {
		return fail("open");
	}

	// Nothing but CONNECT, RESTART_ARM and RESTART goes before a match,
	// and this station has heard no beacon yet.
	const bool refused = tcs_client_ping(client.get()) != 0 &&
		std::string(tcs_last_error()).find("not yet matched") !=
		std::string::npos;
	std::printf("ping before a match: %s\n",
		    refused ? "refused, unsent" : "sent");

	// The first beacon: what the vehicle is flying, before anything has
	// been asked of it.
	tcs_pass pass{};
	do {
		if (tcs_beacons_pass(beacons.get(), &pass) != 0) {
			return fail("beacon");
		}
		for (std::size_t w = 0;
		     w < tcs_beacons_warning_count(beacons.get()); w++) {
			std::cerr << tcs_beacons_warning(beacons.get(), w)
				  << '\n';
		}
	} while (!pass.heard);
	std::printf("beacon: %s%s, %s\n", tcs_stage_label(pass.stage),
		    pass.stage == TCS_STAGE_HEARD && pass.in_contact ?
		    "" : " (not in contact)",
		    tcs_beacons_said(beacons.get()));

	if (!say_matched(configuration.get())) {
		return fail("matched");
	}
	int agrees = 0;
	if (tcs_client_connect(client.get(), &agrees) != 0) {
		return fail("connect");
	}
	std::printf("connect: %s\n",
		    agrees ? "agrees" : "a different configuration");
	if (!say_matched(configuration.get())) {
		return fail("matched");
	}
	if (tcs_client_ping(client.get()) != 0) {
		return fail("ping");
	}
	std::printf("ping: answered\n");

	// A node id means something only once the two ends agree.
	if (agrees) {
		std::uint32_t node = 0;
		tcs_status status = TCS_STATUS_FAILURE;
		int enabled = 0;
		if (tcs_configuration_node_id(configuration.get(),
					      node_name.c_str(), &node) != 0 ||
		    tcs_client_query_node(client.get(), node, &status,
					  &enabled) != 0) {
			return fail("query");
		}
		std::printf("query %s (node %u): %s, %s\n", node_name.c_str(),
			    static_cast<unsigned>(node),
			    tcs_status_name(status),
			    status != TCS_STATUS_SUCCESS ? "refused" :
			    enabled ? "running" : "stopped");

		const std::uint8_t one[] = {'1'};
		if (tcs_client_set_node(client.get(), node, "count", one,
					sizeof one, &status) != 0) {
			return fail("set");
		}
		// Why, where it was refused: the status alone is the same for
		// every refusal, and the reason is what says which word to
		// change.
		const char *reason = tcs_client_reason(client.get());
		std::printf("set %s count: %s%s%s\n", node_name.c_str(),
			    outcome(status).c_str(), *reason ? " -- " : "",
			    reason);

		tcs_sample taken{}, given{};
		if (tcs_client_query_node_sample(client.get(), node, &status,
						 &taken, &given) != 0) {
			return fail("sample");
		}
		const std::string took(
			reinterpret_cast<const char *>(taken.bytes),
			taken.length);
		std::printf("sample %s: took \"%s\" of %u bytes, %s\n",
			    node_name.c_str(), took.c_str(),
			    static_cast<unsigned>(taken.total),
			    given.moved ? "gave something" : "gave nothing");
	}

	if (restart) {
		tcs_status armed = TCS_STATUS_FAILURE,
			   restarted = TCS_STATUS_FAILURE;
		// Onto this configuration, nullptr saying so: a plain
		// restart. Another configuration the vehicle holds is named by
		// its checksum, as tcsverify prints it.
		if (tcs_client_restart_arm(client.get(), ARM_KEY, nullptr,
					   &armed) != 0 ||
		    tcs_client_restart(client.get(), ARM_KEY, nullptr,
				       &restarted) != 0) {
			return fail("restart");
		}
		std::printf("restart: armed %s, restarted %s\n",
			    outcome(armed).c_str(), outcome(restarted).c_str());
		// Nothing more goes to the vehicle until a beacon whose number
		// starts again says it is back: the beacon is the restart's
		// acknowledgement.
		std::printf("restart: awaiting a beacon that starts "
			    "again: %s\n",
			    tcs_client_awaiting_restart(client.get()) == 1
				    ? "yes"
				    : "no");
	}

	// The ping node, through its gateway node, and the vehicle's data from
	// that gateway node: the answer to the ping is the ping node's data, so
	// the two come in together, and whatever else the node sends with
	// it.
	if (tcs_pinger_ping(pinger.get()) != 0) {
		return fail("ping node");
	}
	std::printf("ping %s through %s: sent\n",
		    tcs_pinger_node(pinger.get()),
		    tcs_pinger_gateway(pinger.get()));

	tcs_exchange exchange{};
	bool data_seen = false;
	for (int tries = 0; tries < 10 && !(data_seen && exchange.answered);
	     tries++) {
		tcs_datum datum{};
		const int got =
			tcs_gateway_receive(gateway.get(), 2000, &datum);
		if (got < 0) {
			return fail("data");
		}
		if (got == 0) {
			continue;
		}
		const std::string said(
			reinterpret_cast<const char *>(datum.data),
			datum.length);
		if (std::strcmp(datum.source,
				tcs_pinger_node(pinger.get())) == 0 &&
		    said.rfind("ping #", 0) == 0) {
			if (tcs_pinger_answered(pinger.get(), datum.data,
						datum.length) != 0 ||
			    tcs_pinger_exchange(pinger.get(),
						&exchange) != 0) {
				return fail("answer");
			}
			std::printf("ping %s: answered, round trip %s, up and "
				    "down summing to it %s\n",
				    tcs_pinger_node(pinger.get()),
				    exchange.round_trip >= 0 ?
				    "timed" : "negative",
				    exchange.up + exchange.down ==
				    exchange.round_trip ? "yes" : "no");
		} else {
			data_seen = true;
			std::printf("data from %s: %s\n", datum.source,
				    said.c_str());
		}
	}
	if (!data_seen || !exchange.answered) {
		std::printf("data: not everything arrived\n");
	}

	// Every packet from the vehicle, data and answers, in exactly one of
	// the three counts.
	tcs_counts counts{};
	if (tcs_configuration_counts(configuration.get(), &counts) != 0) {
		return fail("counts");
	}
	std::printf("packets: %llu received = %llu processed + %llu "
		    "malformed + %llu mismatched\n",
		    static_cast<unsigned long long>(counts.received),
		    static_cast<unsigned long long>(counts.processed),
		    static_cast<unsigned long long>(counts.malformed),
		    static_cast<unsigned long long>(counts.mismatched));
	return 0;
}
