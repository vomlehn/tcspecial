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

/** Returns this library's version, as tcspecial's is written.
 *
 * @return  The version, such as "0.1.0"; static, never freed. */
const char *tcs_version(void);

/** Says what went wrong with the last call on this thread that failed.
 *
 * @return  The reason, in words for a person, or "" where no call has
 *          failed; at most TCS_ERROR_BYTES, its NUL included, and good
 *          until the next call on this thread that fails. */
const char *tcs_last_error(void);

/* ---- A configuration ---------------------------------------------------- */

/* A tcspecial configuration, read and checked as the vehicle reads it. */
typedef struct tcs_configuration tcs_configuration;

/** Reads the configuration at path and checks it as the vehicle does.
 *
 * @param path  The configuration file, YAML or XML by its extension.
 * @return      The configuration, or NULL where it cannot be read or the
 *              vehicle would refuse it. */
tcs_configuration *tcs_configuration_open(const char *path);

/** Frees a configuration.
 *
 * @param configuration  The configuration, or NULL to do nothing. */
void tcs_configuration_close(tcs_configuration *configuration);

/** Gives the configuration's digest: what the vehicle announces in every
 * beacon and the answer to a CONNECT.
 *
 * @param configuration  The configuration.
 * @return               The digest, as thirty-two hex digits, or NULL on an
 *                       error. */
const char *tcs_configuration_digest(const tcs_configuration *configuration);

/** Finds the id of a node: its place among the configuration's nodes, which
 * is what a node command names.
 *
 * @param configuration  The configuration.
 * @param name           The node's name.
 * @param id             Where the id is written.
 * @return               0, or -1 where there is no such node. */
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

/** Names a stage as a person reads it.
 *
 * @param stage  The stage.
 * @return       "Unheard", "Heard", "Aging" or "Waiting", or "" for a
 *               number that is no stage; static, never freed. */
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

/** Listens where the configuration says beacons arrive.
 *
 * @param configuration  The configuration, which must have a tcspecial
 *                       section.
 * @return               The beacon watch, or NULL where beacons cannot be
 *                       received. */
tcs_beacons *tcs_beacons_listen(const tcs_configuration *configuration);

/** Waits for a beacon, or for the moment the stage or the indicator would
 * change by itself, and says what is so now. Before any beacon has arrived
 * it waits for one.
 *
 * @param beacons  The beacon watch.
 * @param pass     Where what the pass found is written.
 * @return         0, or -1 on an error. */
int tcs_beacons_pass(tcs_beacons *beacons, tcs_pass *pass);

/** Says what the last beacon said.
 *
 * @param beacons  The beacon watch.
 * @return         "tcspecial v0.1.0 md5 ...", with ", COMMANDED RESTART"
 *                 after it while a commanded restart is under way; "<none>"
 *                 before any beacon; NULL on an error. */
const char *tcs_beacons_said(const tcs_beacons *beacons);

/** Counts what the last pass had to say on a console, which is news:
 * beacons missed, a restart, a configuration that differs.
 *
 * @param beacons  The beacon watch.
 * @return         How many remarks, at most TCS_MAX_REMARKS; 0 on an
 *                 error. */
size_t tcs_beacons_remark_count(const tcs_beacons *beacons);

/** Gives one of the last pass's remarks.
 *
 * @param beacons  The beacon watch.
 * @param index    Which remark, from 0 to tcs_beacons_remark_count() - 1.
 * @return         The remark, or NULL past the end or on an error. */
const char *tcs_beacons_remark(const tcs_beacons *beacons, size_t index);

/** Counts what the last pass found wrong, to be said whether or not anybody
 * asked: a beacon from somewhere beacons do not come from.
 *
 * @param beacons  The beacon watch.
 * @return         How many warnings, at most TCS_MAX_REMARKS; 0 on an
 *                 error. */
size_t tcs_beacons_warning_count(const tcs_beacons *beacons);

/** Gives one of the last pass's warnings.
 *
 * @param beacons  The beacon watch.
 * @param index    Which warning, from 0 to tcs_beacons_warning_count() - 1.
 * @return         The warning, or NULL past the end or on an error. */
const char *tcs_beacons_warning(const tcs_beacons *beacons, size_t index);

/** Stops listening for beacons and frees the watch.
 *
 * @param beacons  The beacon watch, or NULL to do nothing. */
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

/** Opens a moc node on the ground.
 *
 * @param configuration  The configuration.
 * @param moc            The moc node's name.
 * @return               The moc node, or NULL where it cannot be opened. */
tcs_moc *tcs_moc_open(const tcs_configuration *configuration, const char *moc);

/** Takes the next datum from any of the moc node's sources.
 *
 * @param moc      The moc node.
 * @param wait_ms  How long to wait, in milliseconds, or for ever where it is
 *                 negative.
 * @param datum    Where the datum is written; its source and data are the
 *                 moc node's, good until the next receive.
 * @return         1 for a datum, 0 for none in that time, -1 on an
 *                 error. */
int tcs_moc_receive(tcs_moc *moc, int64_t wait_ms, tcs_datum *datum);

/** Closes a moc node.
 *
 * @param moc  The moc node, or NULL to do nothing. */
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

/** Names a status.
 *
 * @param status  The status.
 * @return        "Success", "NotFound" and so on, or "" for a number that
 *                is no status; static, never freed. */
const char *tcs_status_name(tcs_status status);

/* The ground's end of the two command links the configuration states. */
typedef struct tcs_client tcs_client;

/** Opens the two command links the configuration states.
 *
 * @param configuration  The configuration, which must have a tcspecial
 *                       section.
 * @return               The client, or NULL where the links cannot be
 *                       opened. */
tcs_client *tcs_client_open(const tcs_configuration *configuration);

/** Sends CONNECT: says what this end read, and hears what the vehicle read.
 *
 * The answer also says which generation of tcspecial is flying, and clears
 * the commanded-restart bit in the vehicle's beacons. Until a Connect has
 * been answered, and again once a beacon handed to tcs_client_beacon_heard
 * says tcspecial has started again or an answer comes from another
 * generation, every command but PING and CONNECT fails, unsent, because what
 * the vehicle flies may have changed. A command carries no generation, only
 * its sequence. A Connect goes during a wait begun by a RESTART, without
 * ending it.
 *
 * @param client  The client.
 * @param agrees  Where 1 is written when the vehicle is flying this
 *                configuration -- the digests match -- and 0 when it is
 *                not, in which case no node command should be sent.
 * @return        0, or -1 on an error. */
int tcs_client_connect(tcs_client *client, int *agrees);

/** Sends PING, to ask whether the vehicle's command link answers.
 *
 * @param client  The client.
 * @return        0 when it answered, or -1 on an error. */
int tcs_client_ping(tcs_client *client);

/** Sends START_NODE, to start a node.
 *
 * @param client  The client.
 * @param node    The node's id, from tcs_configuration_node_id().
 * @param status  Where what the vehicle answered is written.
 * @return        0, or -1 on an error. */
int tcs_client_start_node(tcs_client *client, uint32_t node,
			  tcs_status *status);

/** Sends STOP_NODE, to stop a node.
 *
 * @param client  The client.
 * @param node    The node's id, from tcs_configuration_node_id().
 * @param status  Where what the vehicle answered is written.
 * @return        0, or -1 on an error. */
int tcs_client_stop_node(tcs_client *client, uint32_t node,
			 tcs_status *status);

/** Sends QUERY_NODE, to ask what a node is doing.
 *
 * @param client   The client.
 * @param node     The node's id, from tcs_configuration_node_id().
 * @param status   Where what the vehicle answered is written.
 * @param enabled  Where 1 is written when the node is running, 0 when not.
 * @return         0, or -1 on an error. */
int tcs_client_query_node(tcs_client *client, uint32_t node,
			  tcs_status *status, int *enabled);

/** Sends RESTART_ARM, to arm a restart.
 *
 * @param client  The client.
 * @param key     The key, which the RESTART that follows must carry.
 * @param status  Where what the vehicle answered is written.
 * @return        0, or -1 on an error. */
int tcs_client_restart_arm(tcs_client *client, uint64_t key,
			   tcs_status *status);

/** Sends RESTART, to restart the vehicle, which the last RESTART_ARM must
 * have armed with the same key: the beacons carry the commanded-restart bit
 * from then, the process restarts, and its beacons start again at #1, the
 * bit still set until a Connect.
 *
 * Look at the beacons before sending it again for want of an answer: they
 * say whether it was taken, and a RESTART that reaches the start that came
 * back restarts it again. From the moment it is sent, every command on this
 * client but CONNECT, RESTART_ARM and RESTART is refused, unsent, until a
 * beacon whose number starts again says tcspecial has started again, and
 * then all but PING and CONNECT until a Connect: the beacon is the
 * acknowledgement, and the answer to the RESTART may never arrive. Hand the
 * client each pass's beacon with tcs_client_beacon_heard. A RESTART
 * answered as refused ends the wait it began at once; one sent again during
 * the wait does not end it either way.
 *
 * @param client  The client.
 * @param key     The key the last RESTART_ARM armed the restart with.
 * @param status  Where what the vehicle answered is written.
 * @return        0, or -1 on an error. */
int tcs_client_restart(tcs_client *client, uint64_t key,
		       tcs_status *status);

/** Hands the client what the last pass of beacons heard, so that a wait
 * begun by a RESTART can end.
 *
 * @param client   The client.
 * @param beacons  The beacon watch, just passed.
 * @return         1 when that ended the wait with the start the RESTART
 *                 commanded; 2 when it ended it with a start nobody
 *                 commanded -- the vehicle restarted for some other reason
 *                 before the RESTART was taken, and runs with the
 *                 definitions it had; 3 when it ended it with a start the
 *                 beacon cannot say of, because a Connect during the wait
 *                 reached that start and cleared its flag; 0 when it did
 *                 not end one; -1 on an error. */
int tcs_client_beacon_heard(tcs_client *client, const tcs_beacons *beacons);

/** Says whether the client is waiting for a beacon to say a RESTART is
 * done, during which every command is refused.
 *
 * @param client  The client.
 * @return        1 when it is, 0 when not, -1 on an error. */
int tcs_client_awaiting_restart(const tcs_client *client);

/** Ends the wait for a RESTART by hand, for an operator who knows the
 * RESTART was never taken.
 *
 * @param client  The client.
 * @return        1 when there was a wait to end, 0 when not, -1 on an
 *                error. */
int tcs_client_end_restart_wait(tcs_client *client);

/* The longest attribute name, and the longest value, a SET_NODE carries, in
 * bytes. Longer is refused here, and not sent. */
#define TCS_ATTRIBUTE_BYTES 64
#define TCS_VALUE_BYTES 256

/** Sends SET_NODE, to set an attribute of a node, which the configuration
 * must let the ground set. Where the vehicle refuses, tcs_client_reason()
 * says why.
 *
 * @param client     The client.
 * @param node       The node's id, from tcs_configuration_node_id().
 * @param attribute  The attribute's name, of at most TCS_ATTRIBUTE_BYTES.
 * @param value      The value's bytes; may be NULL where length is 0.
 * @param length     How many bytes value holds, at most TCS_VALUE_BYTES.
 * @param status     Where what the vehicle answered is written.
 * @return           0, or -1 on an error. */
int tcs_client_set_node(tcs_client *client, uint32_t node,
			const char *attribute, const uint8_t *value,
			size_t length, tcs_status *status);

/** Says why the vehicle refused the last SET_NODE this client sent.
 *
 * @param client  The client.
 * @return        Which attribute no binding names, and its spelling where
 *                that was what was wrong, or what the node would not take;
 *                "" where it did not refuse; NULL on an error. The client's,
 *                good until its next SET_NODE; at most TCS_TEXT_BYTES, its
 *                NUL included. */
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

/** Sends QUERY_NODE_SAMPLE, to ask for the last datum a node took and the
 * last it gave.
 *
 * @param client  The client.
 * @param node    The node's id, from tcs_configuration_node_id().
 * @param status  Where what the vehicle answered is written.
 * @param taken   Where the last datum the node took is written.
 * @param given   Where the last datum the node gave is written.
 * @return        0, or -1 on an error. */
int tcs_client_query_node_sample(tcs_client *client, uint32_t node,
				 tcs_status *status, tcs_sample *taken,
				 tcs_sample *given);

/** Closes the command links and frees the client.
 *
 * @param client  The client, or NULL to do nothing. */
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

/** Opens a way to ping the first ping node the configuration lets the
 * ground reach both ways, through its moc node.
 *
 * @param configuration  The configuration.
 * @return               The pinger, or NULL where there is no such ping
 *                       node or its moc node cannot be reached. */
tcs_pinger *tcs_pinger_open(const tcs_configuration *configuration);

/** Names the ping node, which a datum from tcs_moc_receive() is an answer
 * from.
 *
 * @param pinger  The pinger.
 * @return        The ping node's name, or NULL on an error. */
const char *tcs_pinger_node(const tcs_pinger *pinger);

/** Names the moc node the ping node's answers are to be received from.
 *
 * @param pinger  The pinger.
 * @return        The moc node's name, or NULL on an error. */
const char *tcs_pinger_moc(const tcs_pinger *pinger);

/** Sends the next ping, numbered from one.
 *
 * @param pinger  The pinger.
 * @return        0, or -1 on an error. */
int tcs_pinger_ping(tcs_pinger *pinger);

/** Folds in a datum the ping node sent down, which has just arrived.
 *
 * @param pinger  The pinger.
 * @param data    The datum's bytes; may be NULL where length is 0.
 * @param length  How many bytes data holds.
 * @return        0, or -1 on an error. */
int tcs_pinger_answered(tcs_pinger *pinger, const uint8_t *data,
			size_t length);

/** Says what there is to know of the pings now.
 *
 * @param pinger    The pinger.
 * @param exchange  Where it is written.
 * @return          0, or -1 on an error. */
int tcs_pinger_exchange(const tcs_pinger *pinger, tcs_exchange *exchange);

/** Gives the last ping sent, as text.
 *
 * @param pinger  The pinger.
 * @return        The ping, "<none>" before one is sent, or NULL on an
 *                error. */
const char *tcs_pinger_sent(const tcs_pinger *pinger);

/** Gives the last thing received from the ping node, as text.
 *
 * @param pinger  The pinger.
 * @return        What was received, "<none>" before anything has been, or
 *                NULL on an error. */
const char *tcs_pinger_received(const tcs_pinger *pinger);

/** Frees a pinger.
 *
 * @param pinger  The pinger, or NULL to do nothing. */
void tcs_pinger_close(tcs_pinger *pinger);

#ifdef __cplusplus
}
#endif

#endif /* TCS_H */
