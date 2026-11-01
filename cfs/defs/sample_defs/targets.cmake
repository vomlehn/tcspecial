# The mission tcspecial's cfs test builds: cFE's sample mission, one cFS on
# spacecraft 0x42 as processor 1, running SBN with its UDP module beside the
# lab apps. tcspecial is processor 2 of the same spacecraft, as
# examples/cfs/cfs.ytcs states, and is listed in SBN's peer table
# (apps/sbn/fsw/tables/sbn_conf_tbl.c), not here: it is not a cFS.
#
# Packets carry cFS's extended header and message IDs are cFE's version 2
# mapping, the one framing tcspecial speaks: see "What crosses the bus" in
# tcspecial's docs/design.rst.

SET(MISSION_NAME "TcsCfsTest")
SET(SPACECRAFT_ID 0x42)

SET(MISSION_INCLUDE_CCSDSEXT_HEADER TRUE)
SET(MISSION_MSGID_V2 TRUE)

list(APPEND MISSION_GLOBAL_APPLIST sbn sbn_udp)

SET(FT_INSTALL_SUBDIR "host/functional-test")

SET(MISSION_CPUNAMES cpu1)

SET(cpu1_PROCESSORID 1)
SET(cpu1_APPLIST ci_lab to_lab)
SET(cpu1_FILELIST cfe_es_startup.scr)
