/*
 * TcsBridge.cpp -- tcspecial's commander, inside an F´ deployment.
 *
 * Two threads. The component's own, which F´ runs, takes each command and
 * each tick: on a tick it takes the data that has arrived, sends CONNECT
 * while the bridge is not matched, and writes the channels. The beacon
 * watch's, which waits on tcs_beacons_pass(): a pass waits for a beacon,
 * for ever before the first unless it is woken, so it cannot be a tick's
 * work. It leaves what it heard under a lock for the next tick to read.
 */

#include <cstring>

#include "Fw/Types/String.hpp"
#include "Tcs/TcsBridge/TcsBridge.hpp"

namespace Tcs {

namespace {

/* The beacon watch's thread's stack: tcsmoc-c's, which is Rust's, and
 * sized as Rust sizes a thread's, since a pass needs more than F´'s usual
 * 64 KiB. And how long the thread waits before trying again after a pass
 * fails. */
const FwSizeType BEACON_STACK = 2 * 1024 * 1024;
const Fw::TimeInterval BEACON_RETRY(1, 0);

/* What a command is answered with when the bridge is not open. */
const char *const NOT_OPEN =
	"the bridge is not running: its OpenFailed, ModelDiffers or "
	"BaseDiffers event says why";

/** Copies text into a buffer of fixed size, cutting it short to fit.
 *
 * @param to    The buffer.
 * @param size  Its size, its NUL included.
 * @param from  The text, or NULL for none. */
void copy(char *to, size_t size, const char *from)
{
	if (from == nullptr) {
		from = "";
	}
	std::strncpy(to, from, size - 1);
	to[size - 1] = '\0';
}

/** Gives the stage of the beacon watch as the channel has it.
 *
 * @param stage  The stage, as tcsmoc-c.h has it.
 * @return       The stage, as TcsBridge.fpp has it. */
Stage stageOf(tcs_stage stage)
{
	switch (stage) {
	case TCS_STAGE_HEARD:
		return Stage::HEARD;
	case TCS_STAGE_AGING:
		return Stage::AGING;
	case TCS_STAGE_WAITING:
		return Stage::WAITING;
	case TCS_STAGE_UNHEARD:
	default:
		return Stage::UNHEARD;
	}
}

} // namespace

TcsBridge::TcsBridge(const char *name)
	: TcsBridgeComponentBase(name), m_configuration(nullptr),
	  m_beacons(nullptr), m_waker(nullptr), m_gateway(nullptr),
	  m_uplink(nullptr), m_client(nullptr), m_noUplink(),
	  m_open(false), m_heard(), m_stopping(false), m_tick(),
	  m_connectedAt(0), m_connectedNs(0), m_connected(Connected::NEVER),
	  m_datum(), m_cutShort(0), m_notValues(0),
	  m_notDecisions(0), m_notWholes(0), m_said()
{
}

TcsBridge::~TcsBridge()
{
	// The beacon watch's thread has been joined by stop(), or was never
	// started.
	tcs_beacons_waker_close(m_waker);
	tcs_beacons_close(m_beacons);
	tcs_configuration_close(m_configuration);
}

void TcsBridge::configure(const char *configuration)
{
	m_configuration = tcs_configuration_open(configuration);
	if (m_configuration == nullptr) {
		this->log_WARNING_HI_OpenFailed(Fw::String(tcs_last_error()));
		return;
	}
	const char *digest = tcs_configuration_digest(m_configuration);
	if (digest == nullptr ||
	    std::strcmp(digest, TcsBridge_CONFIGURATION_DIGEST) != 0) {
		this->log_WARNING_HI_ModelDiffers(
			Fw::String(digest == nullptr ? "" : digest),
			Fw::String(TcsBridge_CONFIGURATION_DIGEST));
		return;
	}
	if (this->getIdBase() != TcsBridge_ID_BASE) {
		this->log_WARNING_HI_BaseDiffers(
			static_cast<U32>(this->getIdBase()), TcsBridge_ID_BASE);
		return;
	}

	// Bound before anything is asked of the vehicle, so that nothing it
	// sends in answer arrives before there is anywhere for it to go.
	m_beacons = tcs_beacons_listen(m_configuration);
	if (m_beacons != nullptr) {
		m_waker = tcs_beacons_waker_open(m_beacons);
	}
	if (m_waker != nullptr) {
		m_gateway = tcs_gateway_open(m_configuration,
					     TcsBridge_GATEWAY_NODE);
	}
	if (m_gateway != nullptr) {
		m_client = tcs_client_open(m_configuration);
	}
	if (m_client == nullptr) {
		this->log_WARNING_HI_OpenFailed(Fw::String(tcs_last_error()));
		return;
	}
	// The way up, which a gateway node that binds no address and port
	// aboard does not have. Its want fails SEND_DATA alone, saying why,
	// rather than the bridge: such a node is the far end of a bridge
	// that only listens, and is one a configuration may describe.
	m_uplink = tcs_uplink_open(m_configuration, TcsBridge_GATEWAY_NODE);
	if (m_uplink == nullptr) {
		copy(m_noUplink, sizeof m_noUplink, tcs_last_error());
	}

	Os::Task::Arguments arguments(Fw::String("TcsBeacons"),
				      TcsBridge::watchBeacons, this,
				      Os::Task::TASK_PRIORITY_DEFAULT,
				      BEACON_STACK);
	if (m_beaconTask.start(arguments) != Os::Task::OP_OK) {
		this->log_WARNING_HI_OpenFailed(
			Fw::String("the beacon watch's thread did not start"));
		return;
	}
	m_open = true;
}

void TcsBridge::stop()
{
	// The beacon watch's thread is told to stop, woken from the pass it is
	// in, which before any beacon would wait for one for ever, and joined,
	// so that nothing is left running on the handles the destructor frees.
	m_stopping = true;
	if (m_beaconTask.getState() != Os::Task::State::NOT_STARTED) {
		(void)tcs_beacons_wake(m_waker);
		(void)m_beaconTask.join();
	}
	tcs_client_close(m_client);
	m_client = nullptr;
	tcs_gateway_close(m_gateway);
	m_gateway = nullptr;
	tcs_uplink_close(m_uplink);
	m_uplink = nullptr;
	m_open = false;
}

void TcsBridge::watchBeacons(void *bridge)
{
	TcsBridge &self = *static_cast<TcsBridge *>(bridge);
	tcs_pass pass{};

	while (!self.m_stopping) {
		const bool failed =
			tcs_beacons_pass(self.m_beacons, &pass) != 0;
		{
			Os::ScopeLock lock(self.m_heardLock);
			self.m_heard.failed = failed;
			if (failed) {
				copy(self.m_heard.why, sizeof self.m_heard.why,
				     tcs_last_error());
			} else {
				self.m_heard.pass = pass;
				copy(self.m_heard.said,
				     sizeof self.m_heard.said,
				     tcs_beacons_said(self.m_beacons));
			}
		}
		if (failed) {
			(void)Os::Task::delay(BEACON_RETRY);
		}
	}
}

void TcsBridge::schedIn_handler(FwIndexType portNum, U32 context)
{
	(void)portNum;
	(void)context;
	if (!m_open) {
		return;
	}
	{
		Os::ScopeLock lock(m_heardLock);
		m_tick = m_heard;
	}
	if (m_tick.failed) {
		this->log_WARNING_LO_BeaconsFailed(Fw::String(m_tick.why));
	}
	this->takeData();
	this->connectIfDue(m_tick);
	this->writeChannels(m_tick);
}

void TcsBridge::takeData()
{
	for (int taken = 0; taken < DATA_PER_TICK; taken++) {
		tcs_datum datum{};
		const int got = tcs_gateway_receive(m_gateway, 0, &datum);
		if (got == 0) {
			return;
		}
		if (got < 0) {
			this->log_WARNING_LO_ReceiveFailed(
				Fw::String(tcs_last_error()));
			return;
		}
		if (!this->writeDatum(datum)) {
			this->log_WARNING_HI_NoChannel(
				Fw::String(datum.source));
		}
	}
}

const Datum &TcsBridge::asDatum(const tcs_datum &datum)
{
	// The first bytes, as many as a channel holds, and the rest of the
	// channel's bytes cleared, so that a short datum never shows what a
	// longer one before it left.
	DatumBytes &bytes = m_datum.get_bytes();
	for (FwSizeType i = 0; i < DatumBytes::SIZE; i++) {
		bytes[i] = i < datum.length ? datum.data[i] : 0;
	}
	const U32 length = datum.length > UINT32_MAX ?
		UINT32_MAX : static_cast<U32>(datum.length);
	m_datum.set_length(length);
	if (datum.length > DatumBytes::SIZE) {
		m_cutShort++;
		this->log_WARNING_LO_DatumCutShort(Fw::String(datum.source),
						   length);
	}
	return m_datum;
}

bool TcsBridge::asValue(const tcs_datum &datum, const char *unit,
			F64 &value)
{
	// Read by tcsmoc-c, which reads it as the vehicle wrote it, so that
	// what a value is -- its width, its unit's step, its byte order -- is
	// said once. One that is not a value is written to no channel: a
	// number made of whatever four bytes were there would be a reading
	// nobody took.
	double read = 0.0;
	if (tcs_value_in(unit, datum.data, datum.length, &read) != 0) {
		m_notValues++;
		this->log_WARNING_LO_NotAValue(Fw::String(datum.source),
					       Fw::String(tcs_last_error()));
		return false;
	}
	value = read;
	return true;
}

bool TcsBridge::asDecision(const tcs_datum &datum, bool &on)
{
	// Read by tcsmoc-c, so that what a decision is is said once. One
	// that is not a decision is written to no channel: what a channel said
	// was decided would be a guess.
	int read = 0;
	if (tcs_decision_on(datum.data, datum.length, &read) != 0) {
		m_notDecisions++;
		this->log_WARNING_LO_NotADecision(Fw::String(datum.source),
						  Fw::String(tcs_last_error()));
		return false;
	}
	on = read != 0;
	return true;
}

bool TcsBridge::asUnsigned(const tcs_datum &datum, size_t width, U64 &value)
{
	// Read by tcsmoc-c, so that what a whole number is -- its width,
	// its byte order -- is said once. One of another width is written to no
	// channel.
	uint64_t read = 0;
	if (tcs_unsigned_in(datum.data, datum.length, width, &read) != 0) {
		m_notWholes++;
		this->log_WARNING_LO_NotAWhole(Fw::String(datum.source),
					       Fw::String(tcs_last_error()));
		return false;
	}
	value = read;
	return true;
}

bool TcsBridge::asSigned(const tcs_datum &datum, size_t width, I64 &value)
{
	// As asUnsigned(), in two's complement.
	int64_t read = 0;
	if (tcs_signed_in(datum.data, datum.length, width, &read) != 0) {
		m_notWholes++;
		this->log_WARNING_LO_NotAWhole(Fw::String(datum.source),
					       Fw::String(tcs_last_error()));
		return false;
	}
	value = read;
	return true;
}

void TcsBridge::connectIfDue(const Heard &heard)
{
	int listening = 0, commanding = 0;
	if (tcs_configuration_matched(m_configuration, &listening,
				      &commanding) != 0 ||
	    commanding || !heard.pass.heard ||
	    heard.pass.stage != TCS_STAGE_HEARD) {
		return;
	}
	// One CONNECT for each beacon heard while not matched: a beacon is
	// what says there is a vehicle to answer, and how often it comes is
	// how often the vehicle says it may be spoken to.
	if (heard.pass.at_seconds == m_connectedAt &&
	    heard.pass.at_nanos == m_connectedNs) {
		return;
	}
	m_connectedAt = heard.pass.at_seconds;
	m_connectedNs = heard.pass.at_nanos;

	int agrees = 0;
	Connected now = Connected::FAILED;
	if (tcs_client_connect(m_client, &agrees) == 0) {
		now = agrees ? Connected::AGREED : Connected::DIFFERED;
	}
	// Another configuration, again, is no news. A match is, every time:
	// after a restart the CONNECT that agrees is a new match, though the
	// last CONNECT before the restart agreed too.
	if (now == m_connected && now == Connected::DIFFERED) {
		return;
	}
	switch (now) {
	case Connected::AGREED:
		this->log_ACTIVITY_HI_Matched();
		break;
	case Connected::DIFFERED:
		this->log_WARNING_HI_ConfigurationDiffers(
			Fw::String(heard.said));
		break;
	case Connected::FAILED:
	case Connected::NEVER:
	default:
		this->log_WARNING_LO_ConnectFailed(
			Fw::String(tcs_last_error()));
		break;
	}
	m_connected = now;
}

void TcsBridge::writeChannels(const Heard &heard)
{
	this->tlmWrite_BeaconStage(heard.pass.heard ?
		stageOf(heard.pass.stage) : Stage(Stage::UNHEARD));
	this->tlmWrite_RestartFlag(heard.pass.restart_flag != 0);
	// What the beacon says is longer than a channel's text may be, so it
	// is an event, each time it changes.
	if (heard.pass.heard && std::strcmp(heard.said, m_said) != 0) {
		copy(m_said, sizeof m_said, heard.said);
		this->log_ACTIVITY_HI_BeaconSaid(Fw::String(m_said));
	}

	int listening = 0, commanding = 0;
	if (tcs_configuration_matched(m_configuration, &listening,
				      &commanding) == 0) {
		this->tlmWrite_ListeningMatched(listening != 0);
		this->tlmWrite_CommandingMatched(commanding != 0);
	}
	tcs_counts counts{};
	if (tcs_configuration_counts(m_configuration, &counts) == 0) {
		this->tlmWrite_PacketsReceived(counts.received);
		this->tlmWrite_PacketsProcessed(counts.processed);
		this->tlmWrite_PacketsMalformed(counts.malformed);
		this->tlmWrite_PacketsMismatched(counts.mismatched);
	}
	this->tlmWrite_DataCutShort(m_cutShort);
	this->tlmWrite_DataNotValues(m_notValues);
	this->tlmWrite_DataNotDecisions(m_notDecisions);
	this->tlmWrite_DataNotWholes(m_notWholes);
}

bool TcsBridge::open(const char *command, FwOpcodeType opCode, U32 cmdSeq)
{
	if (m_open) {
		return true;
	}
	this->log_WARNING_LO_CommandFailed(Fw::String(command),
					   Fw::String(NOT_OPEN));
	this->cmdResponse_out(opCode, cmdSeq, Fw::CmdResponse::EXECUTION_ERROR);
	return false;
}

bool TcsBridge::found(const char *command, FwOpcodeType opCode, U32 cmdSeq,
		      const Fw::CmdStringArg &node, U32 &id)
{
	if (TcsBridge::nodeId(node.toChar(), id)) {
		return true;
	}
	this->log_WARNING_LO_NoSuchNode(Fw::String(command), node);
	this->cmdResponse_out(opCode, cmdSeq,
			      Fw::CmdResponse::VALIDATION_ERROR);
	return false;
}

void TcsBridge::PING_cmdHandler(FwOpcodeType opCode, U32 cmdSeq)
{
	if (!this->open("PING", opCode, cmdSeq)) {
		return;
	}
	if (tcs_client_ping(m_client) != 0) {
		this->log_WARNING_LO_CommandFailed(
			Fw::String("PING"), Fw::String(tcs_last_error()));
		this->cmdResponse_out(opCode, cmdSeq,
				      Fw::CmdResponse::EXECUTION_ERROR);
		return;
	}
	this->log_ACTIVITY_HI_PingAnswered();
	this->cmdResponse_out(opCode, cmdSeq, Fw::CmdResponse::OK);
}

void TcsBridge::nodeCommand(const char *command,
			    int (*send)(tcs_client *, uint32_t, tcs_status *),
			    FwOpcodeType opCode, U32 cmdSeq,
			    const Fw::CmdStringArg &node)
{
	U32 id = 0;
	if (!this->open(command, opCode, cmdSeq) ||
	    !this->found(command, opCode, cmdSeq, node, id)) {
		return;
	}
	tcs_status status = TCS_STATUS_FAILURE;
	if (send(m_client, id, &status) != 0) {
		this->log_WARNING_LO_CommandFailed(
			Fw::String(command), Fw::String(tcs_last_error()));
		this->cmdResponse_out(opCode, cmdSeq,
				      Fw::CmdResponse::EXECUTION_ERROR);
		return;
	}
	this->log_ACTIVITY_HI_NodeAnswered(Fw::String(command), node,
					   Fw::String(tcs_status_name(status)));
	this->cmdResponse_out(opCode, cmdSeq,
			      status == TCS_STATUS_SUCCESS ?
			      Fw::CmdResponse::OK :
			      Fw::CmdResponse::EXECUTION_ERROR);
}

void TcsBridge::START_NODE_cmdHandler(FwOpcodeType opCode, U32 cmdSeq,
				      const Fw::CmdStringArg &node)
{
	this->nodeCommand("START_NODE", tcs_client_start_node, opCode, cmdSeq,
			  node);
}

void TcsBridge::STOP_NODE_cmdHandler(FwOpcodeType opCode, U32 cmdSeq,
				     const Fw::CmdStringArg &node)
{
	this->nodeCommand("STOP_NODE", tcs_client_stop_node, opCode, cmdSeq,
			  node);
}

void TcsBridge::QUERY_NODE_cmdHandler(FwOpcodeType opCode, U32 cmdSeq,
				      const Fw::CmdStringArg &node)
{
	U32 id = 0;
	if (!this->open("QUERY_NODE", opCode, cmdSeq) ||
	    !this->found("QUERY_NODE", opCode, cmdSeq, node, id)) {
		return;
	}
	tcs_status status = TCS_STATUS_FAILURE;
	int enabled = 0;
	if (tcs_client_query_node(m_client, id, &status, &enabled) != 0) {
		this->log_WARNING_LO_CommandFailed(
			Fw::String("QUERY_NODE"), Fw::String(tcs_last_error()));
		this->cmdResponse_out(opCode, cmdSeq,
				      Fw::CmdResponse::EXECUTION_ERROR);
		return;
	}
	if (status != TCS_STATUS_SUCCESS) {
		this->log_ACTIVITY_HI_NodeAnswered(
			Fw::String("QUERY_NODE"), node,
			Fw::String(tcs_status_name(status)));
		this->cmdResponse_out(opCode, cmdSeq,
				      Fw::CmdResponse::EXECUTION_ERROR);
		return;
	}
	this->log_ACTIVITY_HI_NodeQueried(node, enabled != 0);
	this->cmdResponse_out(opCode, cmdSeq, Fw::CmdResponse::OK);
}

/**
 * Sends QUERY_THREADS as often as it takes to have every thread: the
 * process's memory goes to the Memory channels and a MemoryQueried event,
 * and the thread that has used the most of its stack, as a share of what
 * it was given, to a FullestThread event.
 *
 * @param opCode  The command's opcode, which its response carries.
 * @param cmdSeq  Its sequence, likewise.
 */
void TcsBridge::QUERY_THREADS_cmdHandler(FwOpcodeType opCode, U32 cmdSeq)
{
	if (!this->open("QUERY_THREADS", opCode, cmdSeq)) {
		return;
	}
	tcs_thread threads[TCS_THREADS_PER_ANSWER];
	tcs_thread fullest{};
	bool any = false;
	U32 first = 0;
	U32 total = 0;
	bool told = false;
	do {
		tcs_status status = TCS_STATUS_FAILURE;
		tcs_memory memory{};
		size_t count = 0;
		if (tcs_client_query_threads(m_client, first, &status, &memory,
					     &total, threads, &count) != 0) {
			this->log_WARNING_LO_CommandFailed(
				Fw::String("QUERY_THREADS"),
				Fw::String(tcs_last_error()));
			this->cmdResponse_out(opCode, cmdSeq,
					      Fw::CmdResponse::EXECUTION_ERROR);
			return;
		}
		if (status != TCS_STATUS_SUCCESS) {
			// Refused: what tcspecial answered is why.
			this->log_WARNING_LO_CommandFailed(
				Fw::String("QUERY_THREADS"),
				Fw::String(tcs_status_name(status)));
			this->cmdResponse_out(opCode, cmdSeq,
					      Fw::CmdResponse::EXECUTION_ERROR);
			return;
		}
		// The memory once, from the first answer: what the process had
		// when it was first asked.
		if (!told) {
			this->tlmWrite_MemoryMapped(memory.mapped);
			this->tlmWrite_MemoryResident(memory.resident);
			this->tlmWrite_MemoryLocked(memory.locked);
			this->tlmWrite_MemoryLockLimit(memory.lock_limit);
			this->log_ACTIVITY_HI_MemoryQueried(
				memory.mapped, memory.resident, memory.locked,
				memory.lock_limit, total);
			told = true;
		}
		// The fullest by share, compared without dividing: a/b > c/d
		// where a*d > c*b.
		for (size_t at = 0; at < count; at++) {
			const tcs_thread &thread = threads[at];
			if (!any ||
			    static_cast<U64>(thread.deepest) * fullest.stack >
				    static_cast<U64>(fullest.deepest) *
					    thread.stack) {
				fullest = thread;
				any = true;
			}
		}
		// An answer that tells of none ends it: the threads a vehicle has
		// can change between two questions.
		if (count == 0) {
			break;
		}
		first += static_cast<U32>(count);
	} while (first < total);
	if (any) {
		this->log_ACTIVITY_HI_FullestThread(
			Fw::String(tcs_thread_role_name(fullest.role)),
			fullest.node, fullest.deepest, fullest.stack);
	}
	this->cmdResponse_out(opCode, cmdSeq, Fw::CmdResponse::OK);
}

/**
 * Sends QUERY_CONSOLIDATOR as often as it takes to have every input: the
 * counts go to a ConsolidatorQueried event, and the input that went into a
 * datum held the most often to a QuietestInput event.
 *
 * @param opCode  The command's opcode, which its response carries.
 * @param cmdSeq  Its sequence, likewise.
 * @param node    The consolidator's name.
 */
void TcsBridge::QUERY_CONSOLIDATOR_cmdHandler(FwOpcodeType opCode, U32 cmdSeq,
					      const Fw::CmdStringArg &node)
{
	U32 id = 0;
	if (!this->open("QUERY_CONSOLIDATOR", opCode, cmdSeq) ||
	    !this->found("QUERY_CONSOLIDATOR", opCode, cmdSeq, node, id)) {
		return;
	}
	tcs_input_counts inputs[TCS_INPUTS_PER_ANSWER];
	tcs_consolidator_counts counts{};
	tcs_input_counts quietest{};
	U32 quietest_at = 0;
	bool any = false;
	U32 first = 0;
	U32 total = 0;
	do {
		tcs_status status = TCS_STATUS_FAILURE;
		size_t told = 0;
		if (tcs_client_query_consolidator(m_client, id, first, &status,
						  &counts, &total, inputs,
						  &told) != 0) {
			this->log_WARNING_LO_CommandFailed(
				Fw::String("QUERY_CONSOLIDATOR"),
				Fw::String(tcs_last_error()));
			this->cmdResponse_out(opCode, cmdSeq,
					      Fw::CmdResponse::EXECUTION_ERROR);
			return;
		}
		if (status != TCS_STATUS_SUCCESS) {
			// Refused, as a node that is no consolidator is: what
			// tcspecial answered is why.
			this->log_ACTIVITY_HI_NodeAnswered(
				Fw::String("QUERY_CONSOLIDATOR"), node,
				Fw::String(tcs_status_name(status)));
			this->cmdResponse_out(opCode, cmdSeq,
					      Fw::CmdResponse::EXECUTION_ERROR);
			return;
		}
		for (size_t at = 0; at < told; at++) {
			if (!any || inputs[at].held > quietest.held) {
				quietest = inputs[at];
				quietest_at = first + static_cast<U32>(at);
				any = true;
			}
		}
		// An answer that tells of none ends it.
		if (told == 0) {
			break;
		}
		first += static_cast<U32>(told);
	} while (first < total);
	this->log_ACTIVITY_HI_ConsolidatorQueried(
		node, counts.sent, counts.sent_all_new, counts.sent_interval,
		counts.sent_moc, counts.refused, total);
	if (any) {
		this->log_ACTIVITY_HI_QuietestInput(node, quietest_at,
						    quietest.held,
						    quietest.none);
	}
	this->cmdResponse_out(opCode, cmdSeq, Fw::CmdResponse::OK);
}

void TcsBridge::restartCommand(const char *command,
			       int (*send)(tcs_client *, uint64_t,
					   const char *, tcs_status *),
			       FwOpcodeType opCode, U32 cmdSeq, U64 key,
			       const Fw::CmdStringArg &target)
{
	if (!this->open(command, opCode, cmdSeq)) {
		return;
	}
	// Empty is this configuration's checksum, which tcsmoc-c is told by
	// NULL: a plain restart. Anything else it holds to being a checksum,
	// and refuses unsent where it is not one.
	const char *onto = target.length() == 0 ? nullptr : target.toChar();
	tcs_status status = TCS_STATUS_FAILURE;
	if (send(m_client, key, onto, &status) != 0) {
		this->log_WARNING_LO_CommandFailed(
			Fw::String(command), Fw::String(tcs_last_error()));
		this->cmdResponse_out(opCode, cmdSeq,
				      Fw::CmdResponse::EXECUTION_ERROR);
		return;
	}
	// Nothing is held back after a RESTART taken: the vehicle answers
	// every command but CONNECT and these two Unmatched until the start
	// that comes back is matched, and that start's beacons, carrying the
	// restart flag, end this bridge's match, so that the CONNECT a beacon
	// is due is what matches it again.
	this->log_ACTIVITY_HI_RestartAnswered(
		Fw::String(command), Fw::String(tcs_status_name(status)));
	this->cmdResponse_out(opCode, cmdSeq,
			      status == TCS_STATUS_SUCCESS ?
			      Fw::CmdResponse::OK :
			      Fw::CmdResponse::EXECUTION_ERROR);
}

void TcsBridge::RESTART_ARM_cmdHandler(FwOpcodeType opCode, U32 cmdSeq,
				       U64 key, const Fw::CmdStringArg &target)
{
	this->restartCommand("RESTART_ARM", tcs_client_restart_arm, opCode,
			     cmdSeq, key, target);
}

void TcsBridge::RESTART_cmdHandler(FwOpcodeType opCode, U32 cmdSeq, U64 key,
				   const Fw::CmdStringArg &target)
{
	this->restartCommand("RESTART", tcs_client_restart, opCode, cmdSeq,
			     key, target);
}

void TcsBridge::SEND_DATA_cmdHandler(FwOpcodeType opCode, U32 cmdSeq,
				     const Fw::CmdStringArg &data)
{
	if (!this->open("SEND_DATA", opCode, cmdSeq)) {
		return;
	}
	if (m_uplink == nullptr) {
		this->log_WARNING_LO_CommandFailed(Fw::String("SEND_DATA"),
						   Fw::String(m_noUplink));
		this->cmdResponse_out(opCode, cmdSeq,
				      Fw::CmdResponse::EXECUTION_ERROR);
		return;
	}
	// The text's bytes, as F´ gave them, without its NUL: one datum, in one
	// datagram, refused by tcsmoc-c unless the bridge is matched for
	// commanding.
	const FwSizeType length = data.length();
	if (tcs_uplink_send(m_uplink,
			    reinterpret_cast<const uint8_t *>(data.toChar()),
			    static_cast<size_t>(length)) != 0) {
		this->log_WARNING_LO_CommandFailed(
			Fw::String("SEND_DATA"), Fw::String(tcs_last_error()));
		this->cmdResponse_out(opCode, cmdSeq,
				      Fw::CmdResponse::EXECUTION_ERROR);
		return;
	}
	this->log_ACTIVITY_LO_DataSent(static_cast<U32>(length));
	this->cmdResponse_out(opCode, cmdSeq, Fw::CmdResponse::OK);
}

} // namespace Tcs
