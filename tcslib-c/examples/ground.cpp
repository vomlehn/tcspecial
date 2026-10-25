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
// does only when it means to; the configuration; the moc node to take data
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

// Each handle, closed by its own _close when it goes.
template <typename T, void (*Close)(T *)> struct Closer {
	void operator()(T *handle) const { Close(handle); }
};
using Configuration = std::unique_ptr<
	tcs_configuration,
	Closer<tcs_configuration, tcs_configuration_close>>;
using BeaconWatch =
	std::unique_ptr<tcs_beacons, Closer<tcs_beacons, tcs_beacons_close>>;
using MocNode = std::unique_ptr<tcs_moc, Closer<tcs_moc, tcs_moc_close>>;
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

// What a status means here, by the header's own constants, so that a header
// whose numbers had drifted from the library's would say so.
const char *outcome(tcs_status status)
{
	return status == TCS_STATUS_SUCCESS ? "done" : "refused";
}

} // namespace

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
				  << ": a configuration, a moc node and a "
				     "node may be named, and nothing more\n";
			return 2;
		} else {
			named.push_back(arg);
		}
	}
	const std::string path = named.size() > 0 ? named[0] :
		STEM + "." + (spelling.empty() ? "yaml" : spelling);
	const std::string moc_name = named.size() > 1 ? named[1] : "moc";
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
	MocNode moc(tcs_moc_open(configuration.get(), moc_name.c_str()));
	Client client(tcs_client_open(configuration.get()));
	PingerHandle pinger(tcs_pinger_open(configuration.get()));
	if (!beacons || !moc || !client || !pinger) {
		return fail("open");
	}

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

	int agrees = 0;
	if (tcs_client_connect(client.get(), &agrees) != 0) {
		return fail("connect");
	}
	std::printf("connect: %s\n",
		    agrees ? "agrees" : "a different configuration");
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
			    outcome(status), *reason ? " -- " : "", reason);

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

	tcs_status configured = TCS_STATUS_FAILURE;
	if (tcs_client_configure(client.get(), 5000000, &configured) != 0) {
		return fail("configure");
	}
	std::printf("configure: beacon every 5 s, %s\n", outcome(configured));

	if (restart) {
		tcs_status armed = TCS_STATUS_FAILURE,
			   restarted = TCS_STATUS_FAILURE;
		if (tcs_client_restart_arm(client.get(), ARM_KEY,
					   &armed) != 0 ||
		    tcs_client_restart(client.get(), ARM_KEY,
				       &restarted) != 0) {
			return fail("restart");
		}
		std::printf("restart: armed %s, restarted %s\n",
			    outcome(armed), outcome(restarted));
	}

	// The ping node, through its moc node, and the vehicle's data from
	// that moc node: the answer to the ping is the ping node's data, so
	// the two come in together, and whatever else the node sends with
	// it.
	if (tcs_pinger_ping(pinger.get()) != 0) {
		return fail("ping node");
	}
	std::printf("ping %s through %s: sent\n",
		    tcs_pinger_node(pinger.get()),
		    tcs_pinger_moc(pinger.get()));

	tcs_exchange exchange{};
	bool data_seen = false;
	for (int tries = 0; tries < 10 && !(data_seen && exchange.answered);
	     tries++) {
		tcs_datum datum{};
		const int got = tcs_moc_receive(moc.get(), 2000, &datum);
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
	return 0;
}
