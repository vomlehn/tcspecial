# What SBN v7.0.1 needs of cFE that the cFE pinned by the cFS checkout calls
# by another name. Included from arch_build_custom.cmake, which prepare
# makes include it, so that SBN's own source is built as it was released.

# cFE renamed CFE_MISSION_SB_SUB_ENTRIES_PER_PKT, the most subscriptions
# one of SB's subscription reports holds, CFE_SB_SUB_ENTRIES_PER_PKT
# (default_cfe_sb_extern_typedefs.h).
add_definitions(
    -DCFE_MISSION_SB_SUB_ENTRIES_PER_PKT=CFE_SB_SUB_ENTRIES_PER_PKT
)
