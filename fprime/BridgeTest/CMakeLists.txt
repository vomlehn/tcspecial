# BridgeTest: the F´ deployment the bridge is tested in, against tcspecial
# and tcs-sim. See the test target in fprime/Makefile.

add_fprime_subdirectory("${CMAKE_CURRENT_LIST_DIR}/Top/")

register_fprime_deployment(
	SOURCES
		"${CMAKE_CURRENT_LIST_DIR}/Main.cpp"
	DEPENDS
		${FPRIME_CURRENT_MODULE}_Top
)
