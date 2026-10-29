/*
 * tcs.h -- tcslib for C and C++.
 *
 * What a ground station is built on, behind one header: the vehicle's beacons,
 * the vehicle's data from a moc node, and commands to the vehicle. The same
 * library the Rust ground programs use -- tcslib -- so a C or C++ station reads
 * the configuration, counts ports and ages beacons exactly as tcsmoc does.
 *
 * C and C++ alike. The header is C, includable from C++ as it stands; every
 * declaration has C linkage, and nothing in it is C that C++ will not compile.
 *
 * Link with libtcs.a (or libtcs.so), and on Linux with -lpthread -ldl -lm.
 *
 * Conventions, the same for every function:
 *
 *   - Each thing a station holds -- a configuration, a beacon watch, a moc
 *     node, a client -- is a handle, made by an _open (or _listen) function
 *     and freed by the matching _close. A _close of NULL does nothing.
 *   - A function that makes a handle returns NULL where it cannot; one that
 *     returns int returns 0 for done and -1 for not done. Either way
 *     tcs_last_error() then says what went wrong, in words for a person.
 *   - Strings are UTF-8 and NUL-terminated. A string a function returns
 *     belongs to the handle it came from, and stays good until the next call
 *     on that handle or its _close; copy it to keep it.
 *   - A handle is used from one thread at a time. tcs_last_error() is per
 *     thread.
 *   - Text the library hands back is held in buffers of fixed size, so that
 *     it allocates nothing of its own as a station runs: see the sizes
 *     below. Text too long for its buffer is cut short at a character and
 *     ends "...".
 */

#ifndef TCS_H
#define TCS_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ---- The library -------------------------------------------------------- */

/* The size of the buffer tcs_last_error() returns, its NUL included. */
#define TCS_ERROR_BYTES 512

/* The size of each other piece of text the library returns -- what a beacon
 * said, each remark and warning -- its NUL included. */
#define TCS_TEXT_BYTES 256

/* How many remarks, and how many warnings, one beacon pass keeps. A pass that
 * has more to say keeps the first TCS_MAX_REMARKS - 1, and its last says how
 * many more there were: "... and 3 more". */
#define TCS_MAX_REMARKS 8

/* This library's version, as tcspecial's is written: "0.1.0". */
const char *tcs_version(void);

/* What went wrong with the last call on this thread that failed, or "" where
 * none has. */
const char *tcs_last_error(void);

/* ---- A configuration ---------------------------------------------------- */

/* A tcspecial configuration, read and checked as the vehicle reads it. */
typedef struct tcs_configuration tcs_configuration;

/* Read the configuration at path, YAML or XML by its extension. */
tcs_configuration *tcs_configuration_open(const char *path);

void tcs_configuration_close(tcs_configuration *configuration);

/* The configuration's digest, as thirty-two hex digits: what the vehicle
 * announces in every beacon and the answer to a CONNECT. */
const char *tcs_configuration_digest(const tcs_configuration *configuration);

/* The id of the node called name -- its place among the configuration's
 * nodes, which is what a node command names -- into *id. */
int tcs_configuration_node_id(const tcs_configuration *configuration,
			      const char *name, uint32_t *id);

/* ---- Beacons ------------------------------------------------------------ */

/* How long it has been since the vehicle's last beacon arrived, counted in
 * beacons missed in a row at the configuration's beacon.default_interval. */
typedef enum tcs_stage {
	TCS_STAGE_UNHEARD = 0, /* nothing has arrived, or nothing can be said */
	TCS_STAGE_HEARD = 1,   /* fewer than four missed: being heard from */
	TCS_STAGE_AGING = 2,   /* four to seven missed */
	TCS_STAGE_WAITING = 3  /* eight or more: waiting to be heard from
	                          again */
} tcs_stage;

/* What a stage is called where a person reads it: "Unheard", "Heard",
 * "Aging" or "Waiting". */
const char *tcs_stage_label(tcs_stage stage);

/* What one pass of the beacon watch found. */
typedef struct tcs_pass {
	tcs_stage stage;
	int lit;          /* whether an indicator showing the stage is lit
	                     now */
	int in_contact;   /* whether the spacecraft is being heard from */
	int heard;        /* whether any beacon has arrived */
	int64_t at_seconds; /* when the last one arrived: seconds since 1970, */
	uint32_t at_nanos;  /* and nanoseconds; both 0 where none has */
	int restarted;    /* whether the beacon this pass heard says tcspecial
	                     has started again: its number went back */
	int commanded_restart; /* whether the last beacon said a restart the
	                          ground commanded is under way: set from the
	                          RESTART until the vehicle takes a CONNECT */
} tcs_pass;

/* The ground's end of the beacon link: a socket where the configuration says
 * beacons arrive, taking them only from where it says they come from. */
typedef struct tcs_beacons tcs_beacons;

/* Listen where the configuration says beacons arrive. */
tcs_beacons *tcs_beacons_listen(const tcs_configuration *configuration);

/* Wait for a beacon, or for the moment the stage or the indicator would
 * change by itself, and say what is so now. Before any beacon has arrived it
 * waits for one. */
int tcs_beacons_pass(tcs_beacons *beacons, tcs_pass *pass);

/* What the last beacon said: "tcspecial v0.1.0 md5 ...", with
 * ", COMMANDED RESTART" after it while a commanded restart is under way, or
 * "<none>". */
const char *tcs_beacons_said(const tcs_beacons *beacons);

/* What the last pass had to say on a console, which is news: beacons missed,
 * a restart, a configuration that differs. */
size_t tcs_beacons_remark_count(const tcs_beacons *beacons);
const char *tcs_beacons_remark(const tcs_beacons *beacons, size_t index);

/* And what is wrong, to be said whether or not anybody asked: a beacon from
 * somewhere beacons do not come from. */
size_t tcs_beacons_warning_count(const tcs_beacons *beacons);
const char *tcs_beacons_warning(const tcs_beacons *beacons, size_t index);

void tcs_beacons_close(tcs_beacons *beacons);

/* ---- The vehicle's data ------------------------------------------------- */

/* A moc node, open on the ground: a port for each node drawn into it. */
typedef struct tcs_moc tcs_moc;

/* One datum, and the node that produced it. */
typedef struct tcs_datum {
	const char *source;  /* the node's name */
	const uint8_t *data; /* what it produced, exactly as sent */
	size_t length;
} tcs_datum;

/* Open the moc node called moc. */
tcs_moc *tcs_moc_open(const tcs_configuration *configuration, const char *moc);

/* The next datum from any source, waiting up to wait_ms milliseconds, or for
 * ever where wait_ms is negative: 1 for a datum, 0 for none in that time,
 * -1 for an error. */
int tcs_moc_receive(tcs_moc *moc, int64_t wait_ms, tcs_datum *datum);

void tcs_moc_close(tcs_moc *moc);

/* ---- Commands ----------------------------------------------------------- */

/* What the vehicle answered a command with. */
typedef enum tcs_status {
	TCS_STATUS_SUCCESS = 0,
	TCS_STATUS_FAILURE = 1,
	TCS_STATUS_INVALID_COMMAND = 2,
	TCS_STATUS_INVALID_PARAMETER = 3,
	TCS_STATUS_NOT_ARMED = 4,
	TCS_STATUS_NOT_FOUND = 5,
	TCS_STATUS_ALREADY_EXISTS = 6,
	TCS_STATUS_TIMEOUT = 7,
	/* 8 is not used: it was a command sent for another generation, when a
	 * command carried one. */
	/* Older than a command the vehicle has already taken since it last
	 * started, and not acted on: overtaken on the link by a later one, or
	 * sent by a client whose count has gone back. The client starts its
	 * count again past the newest the vehicle has taken at its next
	 * Connect. */
	TCS_STATUS_STALE = 9
} tcs_status;

/* A status's name: "Success", "NotFound" and so on. */
const char *tcs_status_name(tcs_status status);

/* The ground's end of the two command links the configuration states. */
typedef struct tcs_client tcs_client;

tcs_client *tcs_client_open(const tcs_configuration *configuration);

/* Say what this end read, and hear what the vehicle read: *agrees is 1 where
 * the vehicle is flying this configuration -- the digests match -- and 0
 * where it is not, in which case no node command should be sent.
 *
 * The answer also says which generation of tcspecial is flying, and clears
 * the commanded-restart bit in the vehicle's beacons. Until a Connect has
 * been answered, and again once a beacon handed to tcs_client_beacon_heard
 * says tcspecial has started again or an answer comes from another
 * generation, every command but PING and CONNECT fails, unsent, because what
 * the vehicle flies may have changed. A command carries no generation, only
 * its sequence. */
int tcs_client_connect(tcs_client *client, int *agrees);

/* Ask whether the vehicle's command link answers. */
int tcs_client_ping(tcs_client *client);

/* Start or stop the node whose id is node, saying what the vehicle answered
 * in *status. */
int tcs_client_start_node(tcs_client *client, uint32_t node,
			  tcs_status *status);
int tcs_client_stop_node(tcs_client *client, uint32_t node,
			 tcs_status *status);

/* Ask what the node whose id is node is doing: *enabled is 1 where it is
 * running. */
int tcs_client_query_node(tcs_client *client, uint32_t node,
			  tcs_status *status, int *enabled);

/* Arm a restart with key, which the RESTART that follows must carry. */
int tcs_client_restart_arm(tcs_client *client, uint64_t key,
			   tcs_status *status);

/* Restart the vehicle, which the last RESTART_ARM must have armed with the
 * same key: the beacons carry the commanded-restart bit from then, the
 * process restarts, and its beacons start again at #1, the bit still set
 * until a Connect.
 *
 * Never send it again for want of an answer: the beacons say whether it was
 * taken. From the moment it is sent, every command on this client is
 * refused, unsent, until a beacon whose number starts again says tcspecial
 * has started again, and then all but PING and CONNECT until a Connect: the beacon is the acknowledgement, and the
 * answer to the RESTART may never arrive. Hand the client each pass's beacon
 * with tcs_client_beacon_heard. A RESTART answered as refused ends the wait
 * at once. */
int tcs_client_restart(tcs_client *client, uint64_t key,
		       tcs_status *status);

/* Hand the client what the last pass of beacons heard: 1 when that ended a
 * wait begun by a RESTART with the start the RESTART commanded; 2 when it
 * ended it with a start nobody commanded -- the vehicle restarted for some
 * other reason before the RESTART was taken, and runs with the definitions
 * it had; 0 when it did not end one; -1 on an error. */
int tcs_client_beacon_heard(tcs_client *client, const tcs_beacons *beacons);

/* Whether the client is waiting for a beacon to say a RESTART is done: 1 or
 * 0, or -1 on an error. */
int tcs_client_awaiting_restart(const tcs_client *client);

/* End that wait by hand, for an operator who knows the RESTART was never
 * taken: 1 when there was one to end, 0 when not, -1 on an error. */
int tcs_client_end_restart_wait(tcs_client *client);

/* Change how often the vehicle beacons, in microseconds. A station's beacon
 * watch goes on aging beacons against the configuration's interval: see the
 * README's known limits. */
int tcs_client_configure(tcs_client *client, uint64_t beacon_interval_us,
			 tcs_status *status);

/* The longest attribute name, and the longest value, a SET_NODE carries, in
 * bytes. Longer is refused here, and not sent. */
#define TCS_ATTRIBUTE_BYTES 64
#define TCS_VALUE_BYTES 256

/* Set attribute of the node whose id is node to the length bytes at value,
 * which the configuration must let the ground set: an attribute of at most
 * TCS_ATTRIBUTE_BYTES and a value of at most TCS_VALUE_BYTES. */
int tcs_client_set_node(tcs_client *client, uint32_t node,
			const char *attribute, const uint8_t *value,
			size_t length, tcs_status *status);

/* Why the vehicle refused the last SET_NODE this client sent -- which
 * attribute no binding names, and its spelling where that was what was wrong;
 * what the node would not take -- or "" where it did not. The client's, good
 * until its next SET_NODE; at most TCS_TEXT_BYTES, its NUL included. */
const char *tcs_client_reason(const tcs_client *client);

/* How many bytes of a transfer a sample holds: the head of it. */
#define TCS_SAMPLE_BYTES 8

/* The last datum a node moved one way. */
typedef struct tcs_sample {
	int moved;          /* whether one has moved at all */
	int64_t at_micros;  /* when, in microseconds since 1970; 0 where
	                       none has */
	uint32_t total;     /* bytes in the whole transfer, */
	uint8_t length;     /* of which bytes holds the first length */
	uint8_t bytes[TCS_SAMPLE_BYTES];
} tcs_sample;

/* The last datum the node whose id is node took, and the last it gave. */
int tcs_client_query_node_sample(tcs_client *client, uint32_t node,
				 tcs_status *status, tcs_sample *taken,
				 tcs_sample *given);

void tcs_client_close(tcs_client *client);

/* ---- Pinging a ping node ------------------------------------------------ */

/* A ping goes up to a moc node, which hands it to the vehicle's ping node;
 * the answer, with the vehicle's time on the end, comes down as that ping
 * node's data from the same moc node, which a station takes with
 * tcs_moc_receive and hands back here. */
typedef struct tcs_pinger tcs_pinger;

/* What there is to know of the pings, at one moment. Times are nanoseconds
 * since 1970; the three lengths of time are nanoseconds, and the two one-way
 * ones are each read across the ground's clock and the vehicle's, so either
 * may be negative -- their sum is the round trip. */
typedef struct tcs_exchange {
	int sent;              /* whether a ping has been sent */
	int64_t sent_at;       /* when the last went, by the ground's clock */
	int answered;          /* whether the last ping's own answer has
	                          arrived */
	int64_t received_at;   /* when it arrived, by the ground's clock */
	int64_t answered_at;   /* when the vehicle had the ping, by its clock */
	int64_t round_trip;    /* ground sent to ground received */
	int64_t up;            /* ground sent to vehicle had */
	int64_t down;          /* vehicle had to ground received */
} tcs_exchange;

/* Ping the first ping node the configuration lets the ground reach both
 * ways, through its moc node. */
tcs_pinger *tcs_pinger_open(const tcs_configuration *configuration);

/* The ping node's name, which a datum from tcs_moc_receive is an answer
 * from; and the moc node's, which is the one to receive it from. */
const char *tcs_pinger_node(const tcs_pinger *pinger);
const char *tcs_pinger_moc(const tcs_pinger *pinger);

/* Send the next ping, numbered from one. */
int tcs_pinger_ping(tcs_pinger *pinger);

/* Fold in a datum the ping node sent down, which has just arrived. */
int tcs_pinger_answered(tcs_pinger *pinger, const uint8_t *data,
			size_t length);

/* What there is to know now. */
int tcs_pinger_exchange(const tcs_pinger *pinger, tcs_exchange *exchange);

/* The last ping sent, and the last thing received, as text: "<none>" before
 * there is one. */
const char *tcs_pinger_sent(const tcs_pinger *pinger);
const char *tcs_pinger_received(const tcs_pinger *pinger);

void tcs_pinger_close(tcs_pinger *pinger);

#ifdef __cplusplus
}
#endif

#endif /* TCS_H */
