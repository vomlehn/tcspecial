/*
 * The cfs test's software bus: cFE's own settings, but for the highest
 * message ID SB takes.
 *
 * cFE's default, 0x1FFF, is the version 1 mapping's whole range, and SB
 * refuses a subscription to anything above it ("Subscribe Err:Bad Arg").
 * Under the version 2 mapping the high byte is a subsystem, and the
 * examples allot tcspecial subsystem 0x42, 0x4200 to 0x42FF, so SBN could
 * not subscribe on tcspecial's behalf to any of its commands. A mission
 * flying tcspecial over the version 2 mapping sets this at least as high
 * as the last ID of the block it allots it; the whole sixteen bits here.
 */

#include "../cfe/modules/sb/config/default_cfe_sb_internal_cfg.h"

#undef CFE_PLATFORM_SB_HIGHEST_VALID_MSGID
#define CFE_PLATFORM_SB_HIGHEST_VALID_MSGID 0xFFFF
