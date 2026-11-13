/*
 * TcsBridge.hpp -- tcspecial's commander, inside an F´ deployment.
 *
 * To tcspecial the bridge is a ground station built on tcslib-moc-c: it opens
 * the configuration the vehicle flies, takes its beacons, sends CONNECT
 * until the two agree, sends the commands and the data F´ gives it and
 * takes the data the vehicle sends down. To F´ it is an active component: commands in,
 * events and channels out. See TcsBridge.fpp for what each is.
 *
 * Nothing is allocated once the deployment is running. Every handle is
 * made, and every buffer is a member, when configure() is called as the
 * topology is set up; a datum is taken into a member and a channel written
 * from it. What tcslib-moc-c does inside tcslib_moc_c.h's calls is what
   tcslib_moc_c.h says of
 * them.
 */

#ifndef TCS_TCSBRIDGE_HPP
#define TCS_TCSBRIDGE_HPP

#include <atomic>

#include "Os/Mutex.hpp"
#include "Os/Task.hpp"
// Named here, and not left to the component's own header: that includes it
// only where some channel of the model is a Datum, and asDatum() uses it
// whatever the model's channels are.
#include "Tcs/TcsBridge/DatumSerializableAc.hpp"
#include "Tcs/TcsBridge/FppConstantsAc.hpp"
#include "Tcs/TcsBridge/TcsBridgeComponentAc.hpp"
#include "tcslib_moc_c.h"

namespace Tcs {

class TcsBridge final : public TcsBridgeComponentBase {
public:
	/** Makes a bridge that does nothing until configure() opens it.
	 *
	 * @param name  The component instance's name. */
	explicit TcsBridge(const char *name);

	/** Frees the handles configure() made. */
	~TcsBridge() override;

	/** Opens the configuration, holds it to the model, and starts taking
	 * beacons. Called once, as the topology is set up, after the base IDs
	 * are given and before the tasks start. Where anything is wrong the
	 * bridge says so in an event and does nothing after, every command
	 * failing: a bridge flying a configuration its channels were not
	 * written from would put one node's data under another's name.
	 *
	 * @param configuration  The tcspecial configuration the vehicle flies,
	 *                       which must be the one the model was written
	 *                       from. */
	void configure(const char *configuration);

	/** Stops taking beacons, joining the beacon watch's thread, and closes
	 * the command links and the gateway node. Called as the topology is taken down, once the component's own
	 * thread has stopped. */
	void stop();

private:
	/* How many data one tick takes at most, so that a payload sending
	 * faster than the deployment ticks cannot hold the bridge's thread
	 * for ever: what is left waits in the socket for the next tick. */
	static const int DATA_PER_TICK = 64;

	/* What the beacon watch last heard, which its thread writes and the
	 * component's reads, under m_heardLock. */
	struct Heard {
		tcs_pass pass;              /* the last pass */
		char said[TCS_TEXT_BYTES];  /* what the last beacon said */
		bool failed;                /* whether the last pass failed */
		char why[TCS_ERROR_BYTES];  /* why, where it did */
	};

	/* How the last CONNECT came out, so that another configuration is
	 * said once as it is found rather than at every beacon. An agreement
	 * is said every time: a CONNECT goes only while the bridge is not
	 * matched, so each one agreed is a match begun again. */
	enum class Connected { NEVER, AGREED, DIFFERED, FAILED };

	// ---- Handlers ----------------------------------------------------

	void schedIn_handler(FwIndexType portNum, U32 context) override;
	void PING_cmdHandler(FwOpcodeType opCode, U32 cmdSeq) override;
	void START_NODE_cmdHandler(FwOpcodeType opCode, U32 cmdSeq,
				   const Fw::CmdStringArg &node) override;
	void STOP_NODE_cmdHandler(FwOpcodeType opCode, U32 cmdSeq,
				  const Fw::CmdStringArg &node) override;
	void QUERY_NODE_cmdHandler(FwOpcodeType opCode, U32 cmdSeq,
				   const Fw::CmdStringArg &node) override;
	void SEND_DATA_cmdHandler(FwOpcodeType opCode, U32 cmdSeq,
				  const Fw::CmdStringArg &data) override;
	void RESTART_ARM_cmdHandler(FwOpcodeType opCode, U32 cmdSeq, U64 key,
				    const Fw::CmdStringArg &target) override;
	void RESTART_cmdHandler(FwOpcodeType opCode, U32 cmdSeq, U64 key,
				const Fw::CmdStringArg &target) override;

	// ---- The model, written from the configuration --------------------

	/** Writes a datum to the channel of the node that produced it, as
	 * what the configuration says its data are: through asDatum(),
	 * asValue(), asDecision(), asUnsigned() or asSigned(). In
	 * model/TcsBridgeModel.cpp, which tcsverify --fprime-model writes.
	 *
	 * @param datum  The datum, and the node that produced it.
	 * @return       true, or false where the model has no channel for the
	 *               node. */
	bool writeDatum(const tcs_datum &datum);

	/** Finds the device id of a node, by its name. In
	 * model/TcsBridgeModel.cpp, which tcsverify --fprime-model writes.
	 *
	 * @param name  The node's name.
	 * @param id    Where its device id is written.
	 * @return      true, or false where the configuration has no such
	 *              node. */
	static bool nodeId(const char *name, U32 &id);

	// ---- What the model writes a datum's channel with -----------------

	/** Makes a datum the bytes a Datum channel holds: its first
	 * DATUM_BYTES, with its whole length, counting and saying it where
	 * it was longer.
	 *
	 * @param datum  The datum.
	 * @return       The channel's value, a member, good until the next
	 *               datum. */
	const Datum &asDatum(const tcs_datum &datum);

	/** Reads a datum as a value in its unit, as tcslib-moc-c reads one,
	 * counting and saying it where it is not one.
	 *
	 * @param datum  The datum.
	 * @param unit   The unit the configuration says it is in: "K", "V".
	 * @param value  Where the number is written.
	 * @return       true where it was a value. */
	bool asValue(const tcs_datum &datum, const char *unit, F64 &value);

	/** Reads a datum as a decision, on or off, as tcslib-moc-c reads one,
	 * counting and saying it where it is not one.
	 *
	 * @param datum  The datum.
	 * @param on     Where the decision is written, true for on.
	 * @return       true where it was a decision. */
	bool asDecision(const tcs_datum &datum, bool &on);

	/** Reads a datum as an unsigned whole number of its type's width, as
	 * tcslib-moc-c reads one, counting and saying it where it is not one.
	 *
	 * @param datum  The datum.
	 * @param width  How many bytes the number is: 1, 2, 4 or 8.
	 * @param value  Where the number is written.
	 * @return       true where it was one. */
	bool asUnsigned(const tcs_datum &datum, size_t width, U64 &value);

	/** Reads a datum as a signed whole number of its type's width, as
	 * tcslib-moc-c reads one, counting and saying it where it is not one.
	 *
	 * @param datum  The datum.
	 * @param width  How many bytes the number is: 1, 2, 4 or 8.
	 * @param value  Where the number is written.
	 * @return       true where it was one. */
	bool asSigned(const tcs_datum &datum, size_t width, I64 &value);

	// ---- The rest ----------------------------------------------------

	/** Takes beacons until the bridge stops: the beacon watch's thread.
	 *
	 * @param bridge  The bridge. */
	static void watchBeacons(void *bridge);

	/** Takes what data has arrived, up to DATA_PER_TICK, and writes each
	 * datum to its channel. */
	void takeData();

	/** Sends CONNECT, once for each beacon heard while the bridge is not
	 * matched for commanding, and says how it came out when that changes.
	 *
	 * @param heard  What the beacon watch last heard. */
	void connectIfDue(const Heard &heard);

	/** Writes the bridge's own channels.
	 *
	 * @param heard  What the beacon watch last heard. */
	void writeChannels(const Heard &heard);

	/** Says whether the bridge is open, and fails a command where not.
	 *
	 * @param command  The command's name, for the event.
	 * @param opCode   The command's opcode.
	 * @param cmdSeq   Its sequence number.
	 * @return         true where the command may go on. */
	bool open(const char *command, FwOpcodeType opCode, U32 cmdSeq);

	/** Sends START_NODE or STOP_NODE, and answers the F´ command by what
	 * tcspecial answered.
	 *
	 * @param command  The command's name, for its events.
	 * @param send     tcs_client_start_node or tcs_client_stop_node.
	 * @param opCode   The F´ command's opcode.
	 * @param cmdSeq   Its sequence number.
	 * @param node     The node's name, as the command gave it. */
	void nodeCommand(const char *command,
			 int (*send)(tcs_client *, uint32_t, tcs_status *),
			 FwOpcodeType opCode, U32 cmdSeq,
			 const Fw::CmdStringArg &node);

	/** Sends RESTART_ARM or RESTART, and answers the F´ command by what
	 * tcspecial answered.
	 *
	 * @param command  The command's name, for its events.
	 * @param send     tcs_client_restart_arm or tcs_client_restart.
	 * @param opCode   The F´ command's opcode.
	 * @param cmdSeq   Its sequence number.
	 * @param key      The key.
	 * @param target   The target's checksum, or empty for this
	 *                 configuration's. */
	void restartCommand(const char *command,
			    int (*send)(tcs_client *, uint64_t, const char *,
					tcs_status *),
			    FwOpcodeType opCode, U32 cmdSeq, U64 key,
			    const Fw::CmdStringArg &target);

	/** Finds a node's device id, and fails the command where there is no
	 * such node.
	 *
	 * @param command  The command's name, for the event.
	 * @param opCode   The F´ command's opcode.
	 * @param cmdSeq   Its sequence number.
	 * @param node     The node's name, as the command gave it.
	 * @param id       Where the node's device id is written.
	 * @return         true where there is such a node. */
	bool found(const char *command, FwOpcodeType opCode, U32 cmdSeq,
		   const Fw::CmdStringArg &node, U32 &id);

	// The handles, made by configure(). The configuration is read from both
	// threads, as tcslib_moc_c.h allows; the beacon watch only from its
	// own, its waker from stop(), and the rest only from the component's.
	tcs_configuration *m_configuration;
	tcs_beacons *m_beacons;
	tcs_beacons_waker *m_waker;
	tcs_gateway *m_gateway;
	tcs_uplink *m_uplink;    // NULL where the gateway node binds nothing
	tcs_client *m_client;
	char m_noUplink[TCS_ERROR_BYTES]; // why m_uplink is NULL

	// Whether configure() opened everything and found the configuration
	// to be the model's.
	bool m_open;

	// The beacon watch's thread, and what it has heard.
	Os::Task m_beaconTask;
	Os::Mutex m_heardLock;
	Heard m_heard;
	std::atomic<bool> m_stopping;

	// What the component's thread keeps between ticks.
	Heard m_tick;            // a copy of m_heard, taken under the lock
	int64_t m_connectedAt;   // when the beacon last connected on came,
	uint32_t m_connectedNs;  // in seconds and nanoseconds
	Connected m_connected;
	Datum m_datum;           // what each datum is taken into
	U32 m_cutShort;          // data longer than a channel holds
	U32 m_notValues;         // data from a node of values, not one
	U32 m_notDecisions;      // data from a node of decisions, not one
	U32 m_notWholes;         // data from a node of whole numbers, not one
	char m_said[TCS_TEXT_BYTES]; // what the beacon last said, as an event
};

} // namespace Tcs

#endif
