/*
 * TcsBridge.cpp -- tcspecial's commander, inside an F´ deployment.
 *
 * Two threads. The component's own, which F´ runs, takes each command and
 * each tick: on a tick it takes the data that has arrived, sends CONNECT
 * while the bridge is not matched, and writes the channels. The beacon
 * watch's, which waits on tcs_beacons_pass(): a pass waits for a beacon,
 * for ever before the first, so it cannot be a tick's work. It leaves what
 * it heard under a lock for the next tick to read.
 */

#include <cstring>

#include "Fw/Types/String.hpp"
#include "Tcs/TcsBridge/TcsBridge.hpp"

namespace Tcs {

namespace {

/* The beacon watch's thread's stack: tcslib-c's, which is Rust's, and
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
 * @param stage  The stage, as tcs.h has it.
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
	  m_beacons(nullptr), m_gateway(nullptr), m_client(nullptr),
	  m_open(false), m_heard(), m_stopping(false), m_tick(),
	  m_connectedAt(0), m_connectedNs(0), m_connected(Connected::NEVER),
	  m_datum(), m_cutShort(0), m_said()
{
}

TcsBridge::~TcsBridge()
{
	// The beacon watch, and the configuration it was made from, are left
	// for the process's end to free where its thread may still be in a
	// pass, which nothing can interrupt: see stop().
	if (m_beaconTask.getState() != Os::Task::State::RUNNING) {
		tcs_beacons_close(m_beacons);
		tcs_configuration_close(m_configuration);
	}
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
	// The beacon watch's thread is told to stop, and stops at the end of
	// the pass it is in. It is not joined: a pass before any beacon has
	// arrived waits for one for ever, and a deployment that could not stop
	// until the vehicle spoke would be worse than one whose last thread
	// ends with the process.
	m_stopping = true;
	tcs_client_close(m_client);
	m_client = nullptr;
	tcs_gateway_close(m_gateway);
	m_gateway = nullptr;
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

		// The first bytes, as many as a channel holds, and the rest of
		// the channel's bytes cleared, so that a short datum never
		// shows what a longer one before it left.
		DatumBytes &bytes = m_datum.get_bytes();
		for (FwSizeType i = 0; i < DatumBytes::SIZE; i++) {
			bytes[i] = i < datum.length ? datum.data[i] : 0;
		}
		const U32 length = datum.length > UINT32_MAX ?
			UINT32_MAX : static_cast<U32>(datum.length);
		m_datum.set_length(length);
		if (datum.length > DatumBytes::SIZE) {
			m_cutShort++;
			this->log_WARNING_LO_DatumCutShort(
				Fw::String(datum.source), length);
		}
		if (!this->writeDatum(datum.source, m_datum)) {
			this->log_WARNING_HI_NoChannel(
				Fw::String(datum.source));
		}
	}
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
	if (now == m_connected && now != Connected::FAILED) {
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

} // namespace Tcs
