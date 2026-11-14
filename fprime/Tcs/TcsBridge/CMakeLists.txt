# TcsBridge: the component, built from its own FPP and C++ and from the model
# tcs-verify --fprime-model writes into model/, and linked with tcsmoc-c.
#
# tcsmoc-c is cargo's: libtcsmoc_c.a, from the tree this directory is
# in, as `cargo build -p tcsmoc-c` leaves it. TCS_LIBRARY names another,
# such as a release build's.

set(TCSPECIAL_ROOT "${CMAKE_CURRENT_LIST_DIR}/../../.."
	CACHE PATH "The tcspecial tree tcsmoc-c is built in")
set(TCS_LIBRARY "${TCSPECIAL_ROOT}/target/debug/libtcsmoc_c.a"
	CACHE FILEPATH
	"The tcsmoc-c static library the bridge is linked with")

# Without the model there is no component: say how to make it, rather than
# leave the FPP tools to say a file is missing.
if (NOT EXISTS "${CMAKE_CURRENT_LIST_DIR}/model/TcsBridgeModel.fppi")
	message(FATAL_ERROR
		"${CMAKE_CURRENT_LIST_DIR}/model holds no model: write it with "
		"`tcs-verify --fprime-model=${CMAKE_CURRENT_LIST_DIR}/model "
		"<configuration>`, or run `make` in fprime/, which does")
endif()

register_fprime_module(
	AUTOCODER_INPUTS
		"${CMAKE_CURRENT_LIST_DIR}/TcsBridge.fpp"
	SOURCES
		"${CMAKE_CURRENT_LIST_DIR}/TcsBridge.cpp"
		"${CMAKE_CURRENT_LIST_DIR}/model/TcsBridgeModel.cpp"
	HEADERS
		"${CMAKE_CURRENT_LIST_DIR}/TcsBridge.hpp"
)
target_include_directories(${FPRIME_CURRENT_MODULE}
	PUBLIC "${TCSPECIAL_ROOT}/tcsmoc-c/include")
# What tcsmoc-c.h says a program built on libtcsmoc_c.a links with
# besides.
target_link_libraries(${FPRIME_CURRENT_MODULE}
	PUBLIC "${TCS_LIBRARY}" pthread dl m)
