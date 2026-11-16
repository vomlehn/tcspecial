/*
 * A ground station that takes a consolidator's data as it comes down, on
 * tcsmoc-c.h: it waits for the vehicle's beacon, connects, waits to be
 * matched, and prints each datum the consolidator sends, as text, a line to
 * a datum, until it has printed as many as it was asked for.
 *
 * Run as "consolidator-station <configuration> <gateway> <consolidator>
 * <datums>", such as, beside the consolidator example,
 *
 *	consolidator-station examples/consolidator/consolidator.ytcs \
 *		gateway housekeeping 5
 *
 * Written so that it compiles as C and as C++, as tcspecial's flown test of
 * the example builds it: see tcspecial/tests/consolidator.rs.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "tcsmoc-c.h"

/* How many passes of the beacon watch the station waits through to be
 * matched for listening once it has connected. */
#define MOST_PASSES 30

/* How many seconds in a row may pass with no datum from the consolidator
 * before the station gives up. */
#define MOST_QUIET 10

/**
 * Says what went wrong, with the library's reason, and gives the failing
 * status.
 *
 * @param doing  What was being done.
 * @return       1, the status a failure exits with.
 */
static int fail(const char *doing)
{
	fprintf(stderr, "consolidator-station: %s: %s\n", doing,
		tcs_last_error());
	return 1;
}

/**
 * Waits through passes of the beacon watch until the station is matched for
 * listening: until a beacon with its restart flag clear, which the vehicle
 * sends once it has taken a CONNECT with its configuration's checksum.
 *
 * @param configuration  The configuration the station uses.
 * @param beacons        The beacon watch.
 * @return               0 once it is matched, or -1 where a pass fails or
 *                       MOST_PASSES go by without a match.
 */
static int until_matched(const tcs_configuration *configuration,
			 tcs_beacons *beacons)
{
	for (int passes = 0; passes < MOST_PASSES; passes++) {
		tcs_pass pass;
		int listening = 0, commanding = 0;

		if (tcs_beacons_pass(beacons, &pass) != 0)
			return -1;
		if (tcs_configuration_matched(configuration, &listening,
					      &commanding) != 0)
			return -1;
		if (listening)
			return 0;
	}
	return -1;
}

/**
 * Takes the consolidator's data as it comes down and prints each datum, until
 * `datums` of them have been printed.
 *
 * @param argc  How many arguments there are.
 * @param argv  The program, the configuration, the gateway node, the
 *              consolidator and how many datums to print.
 * @return      0, or 1 on a failure, which has been said.
 */
int main(int argc, char **argv)
{
	char text[1024];
	tcs_configuration *configuration;
	tcs_beacons *beacons;
	tcs_gateway *gateway;
	tcs_client *client;
	tcs_consolidation *consolidation;
	tcs_pass pass;
	long datums, printed = 0;
	int quiet = 0, agrees = 0, status = 0;

	if (argc != 5) {
		fprintf(stderr, "usage: consolidator-station <configuration> "
				"<gateway> <consolidator> <datums>\n");
		return 2;
	}
	datums = strtol(argv[4], NULL, 10);
	if (datums <= 0) {
		fprintf(stderr, "consolidator-station: the datums to print "
				"must be a whole number above 0\n");
		return 2;
	}

	configuration = tcs_configuration_open(argv[1]);
	if (configuration == NULL)
		return fail("the configuration");
	/* Bound before anything is asked, so that nothing the vehicle sends
	 * arrives before there is anywhere for it to go. */
	beacons = tcs_beacons_listen(configuration);
	gateway = tcs_gateway_open(configuration, argv[2]);
	client = tcs_client_open(configuration);
	consolidation = tcs_consolidation_open(configuration, argv[3]);
	if (beacons == NULL || gateway == NULL || client == NULL ||
	    consolidation == NULL) {
		status = fail("open");
		goto out;
	}

	/* The first beacon says the vehicle is there; a CONNECT then tells it
	 * the station reads its configuration, and a beacon after says so. */
	do {
		if (tcs_beacons_pass(beacons, &pass) != 0) {
			status = fail("the beacon");
			goto out;
		}
	} while (!pass.heard);
	printf("beacon: %s\n", tcs_beacons_said(beacons));
	if (tcs_client_connect(client, &agrees) != 0) {
		status = fail("connect");
		goto out;
	}
	if (!agrees) {
		fprintf(stderr, "consolidator-station: the vehicle flies "
				"another configuration than %s\n", argv[1]);
		status = 1;
		goto out;
	}
	printf("connect: agrees\n");
	if (until_matched(configuration, beacons) != 0) {
		status = fail("waiting to be matched");
		goto out;
	}
	printf("matched\n");

	while (printed < datums) {
		tcs_datum datum;
		int got = tcs_gateway_receive(gateway, 1000, &datum);

		if (got < 0) {
			status = fail("receiving");
			goto out;
		}
		if (got == 0 || strcmp(datum.source, argv[3]) != 0) {
			if (got == 0 && ++quiet == MOST_QUIET) {
				fprintf(stderr, "consolidator-station: nothing "
						"from %s for %d s\n", argv[3],
					MOST_QUIET);
				status = 1;
				goto out;
			}
			continue;
		}
		quiet = 0;
		if (tcs_consolidation_text(consolidation, datum.data,
					   datum.length, " ", text,
					   sizeof text) < 0) {
			printf("refused: %s\n", tcs_last_error());
		} else {
			printf("datum: %s\n", text);
		}
		fflush(stdout);
		printed++;
	}

out:
	tcs_consolidation_close(consolidation);
	tcs_client_close(client);
	tcs_gateway_close(gateway);
	tcs_beacons_close(beacons);
	tcs_configuration_close(configuration);
	return status;
}
