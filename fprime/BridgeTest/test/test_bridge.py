"""The F´ bridge, tested from F´'s own ground system.

Run by ``bridge-test``, once tcssim stands in for the configuration's
payloads and tcspecial flies it, with BridgeTest started by
``conftest.py`` to run the bridge on it: see those. Each test is one thing
``docs/design.rst`` asks of the bridge, seen as the rest of an F´
deployment sees it -- channels, commands and events, by the names the
dictionary gives them -- and never by looking inside the bridge. In the
order a run has them: matched, then beacons, then data, then commands.

The configuration's digest is handed in as ``TCS_CONFIGURATION_DIGEST``,
as tcsverify prints it, so that what the beacon says can be held to it.
"""

import os

from fprime_gds.common.testing_fw import predicates

BRIDGE = "BridgeTest.tcsBridge"
COMMANDER = "CdhCore.cmdDisp"

# How long, in seconds, a run has to do what is waited for. The vehicle
# beacons every five seconds, and nothing comes down until a beacon has been
# heard and a CONNECT answered, so the first waits are the long ones.
MATCHING = 60
ARRIVING = 30
ANSWERING = 15

# The key restarts are armed with, and a checksum of no configuration the
# vehicle holds.
ARM_KEY = 0x5EED
NOWHERE = "0" * 32


class Contains(predicates.predicate):
    """Whether a channel's text holds some text, as F´'s test API asks a
    predicate of a value.

    Args:
        text: The text to look for.
    """

    def __init__(self, text: str):
        self.text = text

    def __call__(self, item) -> bool:
        return isinstance(item, str) and self.text in item

    def __str__(self) -> str:
        return f"x contains {self.text!r}"


def of_bridge(name: str) -> str:
    """Names a channel, command or event of the bridge's, as the
    dictionary does.

    Args:
        name: The name within the component.

    Returns:
        The name within the deployment.
    """
    return f"{BRIDGE}.{name}"


def node_command(api, command: str, node: str) -> None:
    """Sends a node command to the bridge, and asserts it was dispatched
    and completed.

    Args:
        api:     F´'s test API.
        command: START_NODE, STOP_NODE or QUERY_NODE.
        node:    The node's name.
    """
    api.send_and_assert_command(
        of_bridge(command), [node], timeout=ANSWERING, commander=COMMANDER
    )


def test_the_bridge_connects_and_matches(fprime_test_api):
    fprime_test_api.assert_event(of_bridge("Matched"), timeout=MATCHING)
    # Matched, the vehicle's beacon says it carries this configuration's
    # checksum, and no longer that it is unmatched.
    digest = os.environ["TCS_CONFIGURATION_DIGEST"]
    fprime_test_api.assert_event(
        of_bridge("BeaconSaid"),
        [
            predicates.satisfies_all(
                [
                    Contains(f"md5 {digest}"),
                    predicates.invert(Contains("NOT YET MATCHED")),
                ]
            )
        ],
        timeout=MATCHING,
    )
    fprime_test_api.assert_telemetry(
        of_bridge("CommandingMatched"), value=True, timeout=ARRIVING
    )
    fprime_test_api.assert_telemetry(
        of_bridge("ListeningMatched"), value=True, timeout=ARRIVING
    )


def test_beacons_are_heard(fprime_test_api):
    fprime_test_api.assert_telemetry(
        of_bridge("BeaconStage"), value="HEARD", timeout=ARRIVING
    )
    fprime_test_api.assert_telemetry(
        of_bridge("RestartFlag"), value=False, timeout=ARRIVING
    )


def test_a_simulated_reading_arrives_as_a_channel_value(fprime_test_api):
    reading = fprime_test_api.assert_telemetry(
        of_bridge("thermometer"), timeout=ARRIVING
    )
    value = reading.get_val()
    # Two bytes, i16_be, as cabin_temp reads them and tcssim sends them.
    assert value["length"] == 2, value
    fprime_test_api.assert_telemetry(
        of_bridge("PacketsProcessed"), value=predicates.greater_than(0),
        timeout=ARRIVING,
    )


def test_a_value_arrives_as_a_temperature(fprime_test_api):
    # cabin_temp's readings are values, and its channel a number of
    # kelvin: the cabin, which starts at its environment, 283.15 K, and is
    # warmed only from F´, so is no colder than that, and no hotter than a
    # heater left on for the length of a run could make it.
    fprime_test_api.assert_telemetry(
        of_bridge("cabin_temp"),
        value=predicates.within_range(283.0, 400.0),
        timeout=ARRIVING,
    )
    fprime_test_api.assert_telemetry(
        of_bridge("DataNotValues"), value=0, timeout=ARRIVING
    )


def test_a_ping_is_sent_and_answered(fprime_test_api):
    fprime_test_api.send_and_assert_command(
        of_bridge("PING"), timeout=ANSWERING, commander=COMMANDER
    )
    fprime_test_api.assert_event(
        of_bridge("PingAnswered"), timeout=ANSWERING
    )


def test_a_node_is_stopped_and_started_again(fprime_test_api):
    node_command(fprime_test_api, "QUERY_NODE", "thermometer")
    fprime_test_api.assert_event(
        of_bridge("NodeQueried"), ["thermometer", True], timeout=ANSWERING
    )
    node_command(fprime_test_api, "STOP_NODE", "thermometer")
    fprime_test_api.assert_event(
        of_bridge("NodeAnswered"),
        ["STOP_NODE", "thermometer", "Success"],
        timeout=ANSWERING,
    )
    node_command(fprime_test_api, "QUERY_NODE", "thermometer")
    fprime_test_api.assert_event(
        of_bridge("NodeQueried"), ["thermometer", False], timeout=ANSWERING
    )
    node_command(fprime_test_api, "START_NODE", "thermometer")
    node_command(fprime_test_api, "QUERY_NODE", "thermometer")
    fprime_test_api.assert_event(
        of_bridge("NodeQueried"), ["thermometer", True], timeout=ANSWERING
    )


def test_data_sent_up_drives_the_heater(fprime_test_api):
    # 1 up to fsw, and on to the heater, whose stand-in warms the cabin a
    # kelvin a second while it is on: cabin_temp, read back, climbs past
    # anything the cabin left to itself reaches. Then 0, to turn it off.
    fprime_test_api.send_and_assert_command(
        of_bridge("SEND_DATA"), ["1"], timeout=ANSWERING, commander=COMMANDER
    )
    fprime_test_api.assert_event(
        of_bridge("DataSent"), [1], timeout=ANSWERING
    )
    fprime_test_api.assert_telemetry(
        of_bridge("cabin_temp"),
        value=predicates.greater_than(286.0),
        timeout=ARRIVING,
    )
    fprime_test_api.send_and_assert_command(
        of_bridge("SEND_DATA"), ["0"], timeout=ANSWERING, commander=COMMANDER
    )


def test_a_node_the_configuration_lacks_is_refused_unsent(fprime_test_api):
    fprime_test_api.send_command(of_bridge("QUERY_NODE"), ["nosuch"])
    fprime_test_api.assert_event(
        of_bridge("NoSuchNode"), ["QUERY_NODE", "nosuch"], timeout=ANSWERING
    )


def test_a_restart_onto_a_configuration_not_held_is_refused(fprime_test_api):
    fprime_test_api.send_command(
        of_bridge("RESTART_ARM"), [ARM_KEY, NOWHERE]
    )
    fprime_test_api.assert_event(
        of_bridge("RestartAnswered"),
        ["RESTART_ARM", "NotFound"],
        timeout=ANSWERING,
    )


def test_the_vehicle_restarts_and_is_matched_again(fprime_test_api):
    # Last, for it restarts tcspecial. Armed and restarted onto this
    # configuration, which empty names; then the beacons carry the restart
    # flag, and the bridge, connecting by itself, is matched with the start
    # that comes back and commands it.
    for command in ("RESTART_ARM", "RESTART"):
        fprime_test_api.send_and_assert_command(
            of_bridge(command),
            [ARM_KEY, ""],
            timeout=ANSWERING,
            commander=COMMANDER,
        )
        answered = fprime_test_api.assert_event(
            of_bridge("RestartAnswered"),
            [command, "Success"],
            timeout=ANSWERING,
        )
    # From when the RESTART was answered, by the deployment's clock: what
    # the bridge says of the start that comes back may be said before the
    # first of these asks, so it is looked for from then, not from now.
    restarted = answered.get_time()
    # Said as an event, not looked for in the RestartFlag channel: the flag
    # is set only until the bridge's next CONNECT, which may be less than
    # one of the deployment's telemetry periods, and a channel is sampled.
    fprime_test_api.assert_event(
        of_bridge("BeaconSaid"),
        [Contains("RESTARTED")],
        start=restarted,
        timeout=ARRIVING,
    )
    fprime_test_api.assert_event(
        of_bridge("Matched"), start=restarted, timeout=MATCHING
    )
    fprime_test_api.assert_telemetry(
        of_bridge("CommandingMatched"), value=True, start=restarted,
        timeout=ARRIVING,
    )
    fprime_test_api.send_and_assert_command(
        of_bridge("PING"), timeout=ANSWERING, commander=COMMANDER
    )
