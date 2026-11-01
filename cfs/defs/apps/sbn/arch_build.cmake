# SBN's platform configuration, for the cFE the cfs test builds against.
#
# SBN v7.0.1's own arch_build.cmake calls generate_configfile_set, which
# the cFE pinned by the cFS checkout no longer has; this is the same
# generation in the call that cFE has, generate_config_includefile, as the
# lab apps make theirs.

set(SBN_PLATFORM_CONFIG_FILE_LIST
   sbn_msgids.h
)

foreach(SBN_CFGFILE ${SBN_PLATFORM_CONFIG_FILE_LIST})
  get_filename_component(CFGKEY "${SBN_CFGFILE}" NAME_WE)
  if (DEFINED SBN_CFGFILE_SRC_${CFGKEY})
    set(DEFAULT_SOURCE GENERATED_FILE "${SBN_CFGFILE_SRC_${CFGKEY}}")
  else()
    set(DEFAULT_SOURCE
      FALLBACK_FILE "${CMAKE_CURRENT_LIST_DIR}/config/default_${SBN_CFGFILE}")
  endif()
  generate_config_includefile(
    FILE_NAME           "${SBN_CFGFILE}"
    ${DEFAULT_SOURCE}
  )
endforeach()
