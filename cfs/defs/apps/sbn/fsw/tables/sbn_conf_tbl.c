/*
 * SBN's configuration for tcspecial's cfs test: the UDP protocol module and
 * no filter, and two peers on spacecraft 0x42. Processor 1 is this cFS, and
 * its entry is the address SBN binds; processor 2 is tcspecial, at the
 * address examples/cfs/cfs.ytcs gives as sbn.local, which SBN sends to.
 * Both must agree with that file's sbn section, which nothing on the cFS
 * side can check.
 */

#include "sbn_tbl.h"
#include "cfe_tbl_filedef.h"

SBN_ConfTbl_t SBN_ConfTbl = {.ProtocolModules = {{/* [0] */
                                                  .Name        = "UDP",
                                                  .LibFileName = "/cf/sbn_udp.so",
                                                  .LibSymbol   = "SBN_UDP_Ops",
                                                  .BaseEID     = 0x0100}},
                             .ProtocolCnt     = 1,
                             .FilterCnt       = 0,
                             .Peers =
                                 {
                                     {/* [0]: this cFS */
                                      .ProcessorID  = 1,
                                      .SpacecraftID = 0x42,
                                      .NetNum       = 0,
                                      .ProtocolName = "UDP",
                                      .Address      = "127.0.0.1:2234",
                                      .TaskFlags    = SBN_TASK_POLL},
                                     {/* [1]: tcspecial */
                                      .ProcessorID  = 2,
                                      .SpacecraftID = 0x42,
                                      .NetNum       = 0,
                                      .ProtocolName = "UDP",
                                      .Address      = "127.0.0.1:8634",
                                      .TaskFlags    = SBN_TASK_POLL},
                                 },
                             .PeerCnt = 2};

CFE_TBL_FILEDEF(SBN_ConfTbl, SBN.SBN_ConfTbl, SBN Configuration Table, sbn_conf_tbl.tbl)
