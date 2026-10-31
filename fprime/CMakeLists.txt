# The F´ project in tcspecial's tree: the TcsBridge component, and BridgeTest,
# the deployment it is tested in.
#
# Built with F´'s own tools rather than cargo, by `make` in this directory,
# which writes settings.ini to say where the F´ framework is. See Makefile.

cmake_minimum_required(VERSION 3.26)
cmake_policy(SET CMP0048 NEW)
project(TcsFprime VERSION 1.0.0 LANGUAGES C CXX)

# The framework is built here, from the checkout settings.ini names, and not
# found already built: a project and its framework are one build in F´.
set(FPRIME_INCLUDE_FRAMEWORK_CODE OFF)
find_package(FPrime REQUIRED PATHS "${FPRIME_FRAMEWORK_PATH}")

# This project's code is held to the warnings F´ validates its own against.
fprime_add_validation_flags()
add_subdirectory("${FPRIME_FRAMEWORK_PATH}" "F-Prime")
add_fprime_subdirectory("${CMAKE_CURRENT_LIST_DIR}/Tcs/TcsBridge/")
add_fprime_subdirectory("${CMAKE_CURRENT_LIST_DIR}/BridgeTest/")
