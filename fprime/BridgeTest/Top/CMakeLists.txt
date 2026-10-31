# BridgeTest's topology: the instances, how they are wired, and the code that
# sets them up and takes them down.

register_fprime_module(
	AUTOCODER_INPUTS
		"${CMAKE_CURRENT_LIST_DIR}/instances.fpp"
		"${CMAKE_CURRENT_LIST_DIR}/system.fpp"
		"${CMAKE_CURRENT_LIST_DIR}/topology.fpp"
	SOURCES
		"${CMAKE_CURRENT_LIST_DIR}/BridgeTestTopology.cpp"
	DEPENDS
		Fw_Logger
		Svc_PosixTime
		Tcs_TcsBridge
)
