/*
 * Takes a consolidator's datum apart, as a C or C++ ground station does with
 * one tcs_gateway_receive() gave it: each input's name, type and value, and
 * the datum as text, a member to a line.
 *
 * Run as "consolidation <configuration> <consolidator> <datum in hex>", such
 * as
 *
 *	consolidation tests/consolidator/yamcs.ytcs pack 1234010368 6f74
 *
 * where the datum's hexadecimal digits may be in groups. Written so that it
 * compiles as C and as C++, as tcsmoc-c's tests build it: see
 * tests/c_and_cpp.rs.
 */

#include <ctype.h>
#include <inttypes.h>
#include <stdio.h>
#include <string.h>

#include "tcsmoc-c.h"

/* The most bytes a datum given on the command line may be. */
#define MOST_DATUM 256

/* The most inputs this program takes a consolidation apart into. */
#define MOST_MEMBERS 32

/**
 * Says what went wrong, with the library's reason, and gives the failing
 * status.
 *
 * @param doing  What was being done.
 * @return       1, the status a failure exits with.
 */
static int fail(const char *doing)
{
	fprintf(stderr, "consolidation: %s: %s\n", doing, tcs_last_error());
	return 1;
}

/**
 * Reads hexadecimal digits from the command line into bytes, the arguments
 * one after another and spaces between digits allowed.
 *
 * @param argc   How many arguments there are.
 * @param argv   The arguments.
 * @param first  The first that holds digits.
 * @param out    Where the bytes go: MOST_DATUM of them.
 * @return       How many bytes, or -1 for an odd count of digits, a
 *               character that is no digit, or too many.
 */
static long datum_of(int argc, char **argv, int first, uint8_t *out)
{
	long bytes = 0;
	int high = -1;

	for (int n = first; n < argc; n++) {
		for (const char *at = argv[n]; *at != '\0'; at++) {
			int digit;

			if (isspace((unsigned char)*at))
				continue;
			if (!isxdigit((unsigned char)*at))
				return -1;
			digit = isdigit((unsigned char)*at)
				? *at - '0'
				: tolower((unsigned char)*at) - 'a' + 10;
			if (high < 0) {
				high = digit;
				continue;
			}
			if (bytes == MOST_DATUM)
				return -1;
			out[bytes++] = (uint8_t)(high << 4 | digit);
			high = -1;
		}
	}
	return high < 0 ? bytes : -1;
}

/**
 * Takes the datum given on the command line apart, and prints what it holds.
 *
 * @param argc  How many arguments there are.
 * @param argv  The program, the configuration, the consolidator and the
 *              datum's hexadecimal digits.
 * @return      0, or 1 on a failure, which has been said.
 */
int main(int argc, char **argv)
{
	uint8_t datum[MOST_DATUM];
	tcs_member members[MOST_MEMBERS];
	char text[1024];
	tcs_configuration *configuration;
	tcs_consolidation *consolidation;
	size_t inputs;
	long length;
	int got;

	if (argc < 4) {
		fprintf(stderr, "usage: consolidation <configuration> "
				"<consolidator> <datum in hex>...\n");
		return 2;
	}
	length = datum_of(argc, argv, 3, datum);
	if (length < 0) {
		fprintf(stderr, "consolidation: the datum is not whole bytes of "
				"hexadecimal\n");
		return 2;
	}

	configuration = tcs_configuration_open(argv[1]);
	if (configuration == NULL)
		return fail("the configuration");
	consolidation = tcs_consolidation_open(configuration, argv[2]);
	if (consolidation == NULL) {
		tcs_configuration_close(configuration);
		return fail("the consolidator");
	}

	inputs = tcs_consolidation_inputs(consolidation);
	printf("inputs %zu\n", inputs);
	for (size_t at = 0; at < inputs; at++)
		printf("input %s %s\n", tcs_consolidation_input(consolidation, at),
		       tcs_consolidation_type(consolidation, at));

	got = tcs_consolidation_unpack(consolidation, datum, (size_t)length,
				       members, MOST_MEMBERS);
	if (got < 0) {
		printf("refused: %s\n", tcs_last_error());
	} else {
		for (int n = 0; n < got; n++) {
			printf("member %s:", members[n].input);
			for (size_t k = 0; k < members[n].length; k++)
				printf(" %02x", members[n].data[k]);
			printf("\n");
		}
		if (tcs_consolidation_text(consolidation, datum, (size_t)length,
					   "\n", text, sizeof text) < 0) {
			tcs_consolidation_close(consolidation);
			tcs_configuration_close(configuration);
			return fail("the text");
		}
		printf("%s\n", text);
	}

	tcs_consolidation_close(consolidation);
	tcs_configuration_close(configuration);
	return 0;
}
