/*
 * How the cfs test's cFS turns a topic ID into a message ID, in place of
 * cFE's default_cfe_core_api_base_msgids.h, which is written for cFE's
 * version 1 mapping.
 *
 * The build uses the version 2 mapping (MISSION_MSGID_V2), the one
 * tcspecial speaks, in which bit 0x80 of a message ID says it is a
 * command's and the high byte is the extended header's subsystem. cFE's
 * default bases, 0x1800 for commands and 0x0800 for telemetry, are the
 * version 1 mapping's: under version 2 every cFE command would read as
 * telemetry, and SB would refuse SBN's own request for subscription
 * reports. So a command sets 0x80 and telemetry clears it, and the topic's
 * own 0x80 -- the lab apps' topics start there -- moves into the subsystem
 * instead, so that no app's topic lands on a core one. Every message ID
 * this makes is in subsystems 0x18 to 0x1B, clear of tcspecial's 0x42.
 */

#ifndef CFE_CORE_BASE_MSGIDS_H
#define CFE_CORE_BASE_MSGIDS_H

/* The low seven bits of a topic, and its eighth moved up a subsystem. */
#define TCS_TEST_TOPIC_BITS(topic) (((topic)&0x7F) | (((topic)&0x80) << 1))

#define CFE_CPU1_CMD_MID_BASE   0x1880
#define CFE_CPU1_TLM_MID_BASE   0x1800
#define CFE_GLOBAL_CMD_MID_BASE 0x1A80
#define CFE_GLOBAL_TLM_MID_BASE 0x1A00

#define CFE_PLATFORM_CMD_TOPICID_TO_MIDV(topic) \
    (CFE_CPU1_CMD_MID_BASE | TCS_TEST_TOPIC_BITS(topic))
#define CFE_PLATFORM_TLM_TOPICID_TO_MIDV(topic) \
    (CFE_CPU1_TLM_MID_BASE | TCS_TEST_TOPIC_BITS(topic))
#define CFE_GLOBAL_CMD_TOPICID_TO_MIDV(topic) \
    (CFE_GLOBAL_CMD_MID_BASE | TCS_TEST_TOPIC_BITS(topic))
#define CFE_GLOBAL_TLM_TOPICID_TO_MIDV(topic) \
    (CFE_GLOBAL_TLM_MID_BASE | TCS_TEST_TOPIC_BITS(topic))

#endif /* CFE_CORE_BASE_MSGIDS_H */
