/*
 * A ground station in C, on tcs.h.
 *
 * What a ground station does, in the order it does it: read the
 * configuration, wait for the vehicle's first beacon, connect and ping, ask
 * about a node and set one of its attributes, retime the beacon, ping the
 * vehicle's ping node and time the answer, and take the vehicle's data from a
 * moc node. Each step prints one line, and the same program in C++ --
 * ground.cpp -- prints the same lines.
 *
 * Arguments, all optional: --yaml or --xml, but not both, saying which of
 * the example's two configuration files to read (YAML where neither is
 * given); --restart, to arm a restart of the vehicle and restart it, which a
 * station does only when it means to; the configuration; the moc node to take
 * data from; and the node to ask about. Run from the repository's root none
 * need be given, and it watches the ping example.
 *
 *     cc ground.c -Iinclude target/debug/libtcs.a -lpthread -ldl -lm
 */

#include <stdio.h>
#include <string.h>

#include "tcs.h"

/* The example's configuration, without its extension. */
static const char *const STEM = "examples/ping/ping";

/* The key a restart is armed with, which the restart must carry. */
static const uint64_t ARM_KEY = 0x1234;

/* Say what failed, and stop. */
static int fail(const char *what)
{
	fprintf(stderr, "%s: %s\n", what, tcs_last_error());
	return 1;
}

/* What a status means here, by the header's own constants, so that a header
 * whose numbers had drifted from the library's would say so. */
static const char *outcome(tcs_status status)
{
	return status == TCS_STATUS_SUCCESS ? "done" : "refused";
}

int main(int argc, char **argv)
{
	const char *spelling = NULL;
	const char *named[3] = {NULL, NULL, NULL};
	int count = 0;
	int restart = 0;
	char stem_path[512];

	for (int i = 1; i < argc; i++) {
		if (strcmp(argv[i], "--yaml") == 0 ||
		    strcmp(argv[i], "--xml") == 0) {
			const char *asked = argv[i] + 2;
			if (spelling != NULL && strcmp(spelling, asked) != 0) {
				fprintf(stderr,
					"--yaml and --xml each say which "
					"configuration to read: give one of "
					"them, or neither\n");
				return 2;
			}
			spelling = asked;
		} else if (strcmp(argv[i], "--restart") == 0) {
			restart = 1;
		} else if (argv[i][0] == '-') {
			fprintf(stderr,
				"%s is not an option: the options are "
				"--yaml, --xml and --restart\n", argv[i]);
			return 2;
		} else if (count == 3) {
			fprintf(stderr,
				"%s: a configuration, a moc node and a node "
				"may be named, and nothing more\n", argv[i]);
			return 2;
		} else {
			named[count++] = argv[i];
		}
	}
	if (named[0] == NULL) {
		snprintf(stem_path, sizeof stem_path, "%s.%s", STEM,
			 spelling ? spelling : "yaml");
		named[0] = stem_path;
	}
	const char *path = named[0];
	const char *moc_name = named[1] ? named[1] : "moc";
	const char *node_name = named[2] ? named[2] : "ping";

	tcs_configuration *configuration = tcs_configuration_open(path);
	if (configuration == NULL) {
		return fail("configuration");
	}
	printf("configuration: md5 %s\n",
	       tcs_configuration_digest(configuration));

	/* Bound before anything is asked, so that nothing the vehicle sends in
	 * answer arrives before there is anywhere for it to go. */
	tcs_beacons *beacons = tcs_beacons_listen(configuration);
	tcs_moc *moc = tcs_moc_open(configuration, moc_name);
	tcs_client *client = tcs_client_open(configuration);
	tcs_pinger *pinger = tcs_pinger_open(configuration);
	if (beacons == NULL || moc == NULL || client == NULL ||
	    pinger == NULL) {
		return fail("open");
	}

	/* The first beacon: what the vehicle is flying, before anything has
	 * been asked of it. */
	tcs_pass pass;
	do {
		if (tcs_beacons_pass(beacons, &pass) != 0) {
			return fail("beacon");
		}
		for (size_t w = 0; w < tcs_beacons_warning_count(beacons);
		     w++) {
			fprintf(stderr, "%s\n",
				tcs_beacons_warning(beacons, w));
		}
	} while (!pass.heard);
	printf("beacon: %s%s, %s\n", tcs_stage_label(pass.stage),
	       pass.stage == TCS_STAGE_HEARD && pass.in_contact ?
	       "" : " (not in contact)",
	       tcs_beacons_said(beacons));

	int agrees = 0;
	if (tcs_client_connect(client, &agrees) != 0) {
		return fail("connect");
	}
	printf("connect: %s\n",
	       agrees ? "agrees" : "a different configuration");
	if (tcs_client_ping(client) != 0) {
		return fail("ping");
	}
	printf("ping: answered\n");

	/* A node id means something only once the two ends agree. */
	if (agrees) {
		uint32_t node = 0;
		tcs_status status = TCS_STATUS_FAILURE;
		int enabled = 0;
		if (tcs_configuration_node_id(configuration, node_name,
					      &node) != 0
		    || tcs_client_query_node(client, node, &status,
					     &enabled) != 0) {
			return fail("query");
		}
		printf("query %s (node %u): %s, %s\n", node_name,
		       (unsigned)node, tcs_status_name(status),
		       status != TCS_STATUS_SUCCESS ? "refused" :
		       enabled ? "running" : "stopped");

		static const uint8_t one[] = {'1'};
		if (tcs_client_set_node(client, node, "count", one,
					sizeof one, &status) != 0) {
			return fail("set");
		}
		/* Why, where it was refused: the status alone is the same for
		 * every refusal, and the reason is what says which word to
		 * change. */
		printf("set %s count: %s%s%s\n", node_name, outcome(status),
		       *tcs_client_reason(client) ? " -- " : "",
		       tcs_client_reason(client));

		tcs_sample taken, given;
		if (tcs_client_query_node_sample(client, node, &status,
						 &taken, &given) != 0) {
			return fail("sample");
		}
		printf("sample %s: took \"%.*s\" of %u bytes, %s\n",
		       node_name, (int)taken.length,
		       (const char *)taken.bytes, (unsigned)taken.total,
		       given.moved ? "gave something" : "gave nothing");
	}

	tcs_status configured = TCS_STATUS_FAILURE;
	if (tcs_client_configure(client, 5000000, &configured) != 0) {
		return fail("configure");
	}
	printf("configure: beacon every 5 s, %s\n", outcome(configured));

	if (restart) {
		tcs_status armed = TCS_STATUS_FAILURE,
			   restarted = TCS_STATUS_FAILURE;
		if (tcs_client_restart_arm(client, ARM_KEY, &armed) != 0
		    || tcs_client_restart(client, ARM_KEY,
					  &restarted) != 0) {
			return fail("restart");
		}
		printf("restart: armed %s, restarted %s\n", outcome(armed),
		       outcome(restarted));
	}

	/* The ping node, through its moc node, and the vehicle's data from
	 * that moc node: the answer to the ping is the ping node's data, so
	 * the two come in together, and whatever else the node sends with
	 * it. */
	if (tcs_pinger_ping(pinger) != 0) {
		return fail("ping node");
	}
	printf("ping %s through %s: sent\n", tcs_pinger_node(pinger),
	       tcs_pinger_moc(pinger));

	tcs_exchange exchange;
	int data_seen = 0;
	exchange.answered = 0;
	for (int tries = 0; tries < 10 && !(data_seen && exchange.answered);
	     tries++) {
		tcs_datum datum;
		int got = tcs_moc_receive(moc, 2000, &datum);
		if (got < 0) {
			return fail("data");
		}
		if (got == 0) {
			continue;
		}
		if (strcmp(datum.source, tcs_pinger_node(pinger)) == 0
		    && strncmp((const char *)datum.data, "ping #", 6) == 0) {
			if (tcs_pinger_answered(pinger, datum.data,
						datum.length) != 0
			    || tcs_pinger_exchange(pinger, &exchange) != 0) {
				return fail("answer");
			}
			printf("ping %s: answered, round trip %s, up and down "
			       "summing to it %s\n",
			       tcs_pinger_node(pinger),
			       exchange.round_trip >= 0 ? "timed" : "negative",
			       exchange.up + exchange.down ==
			       exchange.round_trip ? "yes" : "no");
		} else {
			data_seen = 1;
			printf("data from %s: %.*s\n", datum.source,
			       (int)datum.length, (const char *)datum.data);
		}
	}
	if (!data_seen || !exchange.answered) {
		printf("data: not everything arrived\n");
	}

	tcs_pinger_close(pinger);
	tcs_client_close(client);
	tcs_moc_close(moc);
	tcs_beacons_close(beacons);
	tcs_configuration_close(configuration);
	return 0;
}
