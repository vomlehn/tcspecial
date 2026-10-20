// A ground station in C++, on tcs.h.
//
// The same program as ground.c, written as C++ is: the header included as it
// stands, each handle held by a std::unique_ptr that closes it, and strings
// as std::string. It prints the same lines, which is what the test holds the
// two to.
//
// Arguments, all optional: --yaml or --xml, but not both, saying which of the
// example's two configuration files to read (YAML where neither is given);
// the configuration; the moc node to take data from; and the node to ask
// about. Run from the repository's root none need be given, and it watches
// the ping example.
//
//     c++ ground.cpp -Iinclude target/debug/libtcs.a -lpthread -ldl -lm

#include <cstdint>
#include <cstdio>
#include <iostream>
#include <memory>
#include <string>
#include <vector>

#include "tcs.h"

namespace {

// The example's configuration, without its extension.
const std::string STEM = "examples/ping/ping";

// Each handle, closed by its own _close when it goes.
template <typename T, void (*Close)(T *)> struct Closer {
    void operator()(T *handle) const { Close(handle); }
};
using Configuration =
    std::unique_ptr<tcs_configuration, Closer<tcs_configuration, tcs_configuration_close>>;
using BeaconWatch = std::unique_ptr<tcs_beacons, Closer<tcs_beacons, tcs_beacons_close>>;
using MocNode = std::unique_ptr<tcs_moc, Closer<tcs_moc, tcs_moc_close>>;
using Client = std::unique_ptr<tcs_client, Closer<tcs_client, tcs_client_close>>;

// Say what failed, and stop.
int fail(const std::string &what)
{
    std::cerr << what << ": " << tcs_last_error() << '\n';
    return 1;
}

} // namespace

int main(int argc, char **argv)
{
    std::string spelling;
    std::vector<std::string> named;

    for (int i = 1; i < argc; i++) {
        const std::string arg = argv[i];
        if (arg == "--yaml" || arg == "--xml") {
            const std::string asked = arg.substr(2);
            if (!spelling.empty() && spelling != asked) {
                std::cerr << "--yaml and --xml each say which configuration to read: give one "
                             "of them, or neither\n";
                return 2;
            }
            spelling = asked;
        } else if (!arg.empty() && arg[0] == '-') {
            std::cerr << arg << " is not an option: the options are --yaml and --xml\n";
            return 2;
        } else if (named.size() == 3) {
            std::cerr << arg
                      << ": a configuration, a moc node and a node may be named, and nothing "
                         "more\n";
            return 2;
        } else {
            named.push_back(arg);
        }
    }
    const std::string path =
        named.size() > 0 ? named[0] : STEM + "." + (spelling.empty() ? "yaml" : spelling);
    const std::string moc_name = named.size() > 1 ? named[1] : "moc";
    const std::string node_name = named.size() > 2 ? named[2] : "ping";

    Configuration configuration(tcs_configuration_open(path.c_str()));
    if (!configuration) {
        return fail("configuration");
    }
    std::printf("configuration: md5 %s\n", tcs_configuration_digest(configuration.get()));

    // Bound before anything is asked, so that nothing the vehicle sends in
    // answer arrives before there is anywhere for it to go.
    BeaconWatch beacons(tcs_beacons_listen(configuration.get()));
    MocNode moc(tcs_moc_open(configuration.get(), moc_name.c_str()));
    Client client(tcs_client_open(configuration.get()));
    if (!beacons || !moc || !client) {
        return fail("open");
    }

    // The first beacon: what the vehicle is flying, before anything has been
    // asked of it.
    tcs_pass pass{};
    do {
        if (tcs_beacons_pass(beacons.get(), &pass) != 0) {
            return fail("beacon");
        }
        for (std::size_t w = 0; w < tcs_beacons_warning_count(beacons.get()); w++) {
            std::cerr << tcs_beacons_warning(beacons.get(), w) << '\n';
        }
    } while (!pass.heard);
    // By the header's own constants, so that a header whose numbers had
    // drifted from the library's would say so here.
    std::printf("beacon: %s%s, %s\n", tcs_stage_label(pass.stage),
                pass.stage == TCS_STAGE_HEARD && pass.in_contact ? "" : " (not in contact)",
                tcs_beacons_said(beacons.get()));

    int agrees = 0;
    if (tcs_client_connect(client.get(), &agrees) != 0) {
        return fail("connect");
    }
    std::printf("connect: %s\n", agrees ? "agrees" : "a different configuration");
    if (tcs_client_ping(client.get()) != 0) {
        return fail("ping");
    }
    std::printf("ping: answered\n");

    // A node id means something only once the two ends agree.
    if (agrees) {
        std::uint32_t node = 0;
        tcs_status status = TCS_STATUS_FAILURE;
        int enabled = 0;
        if (tcs_configuration_node_id(configuration.get(), node_name.c_str(), &node) != 0 ||
            tcs_client_query_node(client.get(), node, &status, &enabled) != 0) {
            return fail("query");
        }
        std::printf("query %s (node %u): %s, %s\n", node_name.c_str(),
                    static_cast<unsigned>(node), tcs_status_name(status),
                    status != TCS_STATUS_SUCCESS ? "refused" : enabled ? "running" : "stopped");
    }

    tcs_datum datum{};
    const int got = tcs_moc_receive(moc.get(), 10000, &datum);
    if (got < 0) {
        return fail("data");
    }
    if (got == 0) {
        std::printf("data: nothing in ten seconds\n");
    } else {
        const std::string said(reinterpret_cast<const char *>(datum.data), datum.length);
        std::printf("data from %s: %s\n", datum.source, said.c_str());
    }
    return 0;
}
