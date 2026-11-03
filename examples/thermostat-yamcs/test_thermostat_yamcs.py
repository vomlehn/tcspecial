"""Tests of thermostat_yamcs.py that need neither Yamcs, tcssim nor a
vehicle: what it reads from the configuration, the mission database it has
generated -- a typed parameter for every node drawn into the gateway node --
and what it shows of a Yamcs played here.

Run from the repository's root::

    python3 -m pytest examples/thermostat-yamcs
"""

import contextlib
import io
import sys
import tempfile
import types
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import thermostat_yamcs  # noqa: E402
from tcslib.beacon import Arrived  # noqa: E402
from tcslibgs import configuration  # noqa: E402
from tcslibgs import yamcs as generated  # noqa: E402
from tcslibgs.form import DATUM, DECISION, UNITS, Form  # noqa: E402

#: The XTCE namespace the wrapper is in, for ElementTree's paths.
X = {"x": generated.NAMESPACE}

#: A value in kelvin, as the readings and the vote are.
KELVIN = Form("Value", UNITS[0])


def options(**changed):
    """The options thermostat_yamcs reads, for the example's configuration,
    with no tcsverify asked for: the digest is not what these tests are
    about.

    Args:
        changed: Options to change.

    Returns:
        A namespace like the one :func:`thermostat_yamcs.arguments` gives.
    """
    said = dict(configuration=str(HERE / "thermostat-yamcs.xtcs"),
                tcsverify=None)
    said.update(changed)
    return types.SimpleNamespace(**said)


class SetupTest(unittest.TestCase):
    """What is read out of the example's configuration."""

    def test_the_sources_are_the_readings_the_vote_and_the_heaters(self):
        setup = thermostat_yamcs.Setup(options())
        self.assertEqual(setup.gateway, "gateway")
        self.assertEqual(
            [(source.node, source.parameter, source.form, source.drives)
             for source in setup.sources],
            [("ambient", "/TcsYamcs/ambient", KELVIN, ()),
             # The lines sensing the heaters, each on or off: what the
             # heaters are doing, where the thermostat's decision is not
             # sent down.
             ("gpio-in-tmass line 2", "/TcsYamcs/gpio-in-tmass_line2",
              DECISION, ()),
             ("gpio-in-tmass line 3", "/TcsYamcs/gpio-in-tmass_line3",
              DECISION, ()),
             ("temp1", "/TcsYamcs/temp1", KELVIN, ()),
             ("temp2", "/TcsYamcs/temp2", KELVIN, ()),
             ("temp3", "/TcsYamcs/temp3", KELVIN, ())])

    def test_the_example_says_its_apids_and_ports(self):
        setup = thermostat_yamcs.Setup(options())
        # ambient 0, gateway 1, gpio-in-tmass 2, gpio-out-tmass1 and 2 3
        # and 4, housekeeping 5, temp1 to temp3 6 to 8, temp_1 to temp_3 9
        # to 11, thermostat 12.
        self.assertEqual([(link.source, link.apid, link.ground.port)
                          for link in setup.links],
                         [("ambient", 0x400, 17700),
                          ("gpio-in-tmass", 0x402, 17702),
                          ("temp1", 0x406, 17706), ("temp2", 0x407, 17707),
                          ("temp3", 0x408, 17708)])
        self.assertEqual(setup.config.beacon_apid(), 0x110)
        self.assertEqual(setup.config.command_apid(), 0x111)
        self.assertEqual(setup.beacon.recv.port, 17510)

    def test_both_spellings_say_the_same(self):
        xml = thermostat_yamcs.Setup(options()).describe(Path("/r"), 1)
        yaml = thermostat_yamcs.Setup(options(configuration=str(
            HERE / "thermostat-yamcs.ytcs"))).describe(Path("/r"), 1)
        self.assertEqual(xml, yaml)

    def test_a_radios_configuration_is_refused(self):
        with self.assertRaisesRegex(ValueError, "gateway: yamcs"):
            thermostat_yamcs.Setup(options(configuration=str(
                HERE.parent / "thermostat" / "thermostat.xtcs")))

    def test_every_is_a_number_of_seconds(self):
        with contextlib.redirect_stderr(io.StringIO()), \
                self.assertRaises(SystemExit):
            thermostat_yamcs.arguments(["--every", "0"])
        self.assertEqual(thermostat_yamcs.arguments([]).every, 2.0)


class MdbTest(unittest.TestCase):
    """The mission database generated for the example: a parameter for each
    node drawn into the gateway node, of the type its data are."""

    @classmethod
    def setUpClass(cls):
        config = configuration.load(HERE / "thermostat-yamcs.xtcs")
        cls.root = generated.mdb(config).getroot()

    def kind_of(self, parameter):
        """A parameter's type.

        Args:
            parameter: The parameter's name.

        Returns:
            Its type's element.
        """
        found, = self.root.iterfind(
            f".//x:ParameterSet/x:Parameter[@name='{parameter}']", X)
        name = found.get("parameterTypeRef")
        kind, = self.root.iterfind(
            f".//x:ParameterTypeSet/*[@name='{name}']", X)
        return kind

    def test_the_readings_and_the_vote_are_temperatures(self):
        for parameter in ("temp1", "temp2", "temp3", "ambient"):
            kind = self.kind_of(parameter)
            self.assertEqual(kind.tag, f"{{{generated.NAMESPACE}}}"
                                       "FloatParameterType")
            self.assertEqual(kind.get("name"), "KelvinValueType")
            unit, = kind.iterfind("./x:UnitSet/x:Unit", X)
            self.assertEqual((unit.text, unit.get("description")),
                             ("K", "kelvin"))
            encoding, = kind.iterfind("./x:IntegerDataEncoding", X)
            self.assertEqual(
                (encoding.get("sizeInBits"), encoding.get("encoding"),
                 encoding.get("byteOrder")),
                ("32", "twosComplement", "mostSignificantByteFirst"))
            term, = encoding.iterfind(
                "./x:DefaultCalibrator/x:PolynomialCalibrator/x:Term", X)
            self.assertEqual((float(term.get("coefficient")),
                              term.get("exponent")), (1e-6, "1"))

    def test_each_line_is_a_record_and_its_level_a_boolean(self):
        container, = self.root.iterfind(
            ".//x:SequenceContainer[@name='gpio-in-tmassPacket']", X)
        held = [entry.get("parameterRef") for entry in container.iterfind(
            "./x:EntryList/x:ParameterRefEntry", X)]
        self.assertEqual(held, [
            "gpio-in-tmass_line2_time", "gpio-in-tmass_line2_offset",
            "gpio-in-tmass_line2", "gpio-in-tmass_line3_time",
            "gpio-in-tmass_line3_offset", "gpio-in-tmass_line3"])
        sizes = []
        for name in held:
            encoding, = self.kind_of(name).iterfind(
                "./x:IntegerDataEncoding", X)
            sizes.append(int(encoding.get("sizeInBits")))
        # A record: the time, the offset and the level, sixteen bytes.
        self.assertEqual(sizes, [64, 32, 32] * 2)
        level = self.kind_of("gpio-in-tmass_line2")
        self.assertEqual(level.tag, f"{{{generated.NAMESPACE}}}"
                                    "BooleanParameterType")

    def test_each_type_is_written_once(self):
        names = [kind.get("name") for kind in self.root.iterfind(
            ".//x:ParameterTypeSet/*", X)]
        self.assertEqual(names.count("KelvinValueType"), 1)
        self.assertEqual(names.count(generated.LINE_LEVEL_TYPE), 1)

    def test_each_container_is_its_sources_apid(self):
        found = {}
        for container in self.root.iterfind(
                ".//x:SequenceContainer", X):
            for comparison in container.iterfind(
                    "./x:BaseContainer/x:RestrictionCriteria/x:Comparison", X):
                if comparison.get("parameterRef") == "CcsdsApid":
                    found[container.get("name")] = int(comparison.get("value"))
        self.assertEqual({name: apid for name, apid in found.items()
                          if name.endswith("Packet")
                          and name[:-len("Packet")] in
                          ("ambient", "gpio-in-tmass", "temp1", "temp2",
                           "temp3")},
                         {"ambientPacket": 0x400, "gpio-in-tmassPacket": 0x402,
                          "temp1Packet": 0x406, "temp2Packet": 0x407,
                          "temp3Packet": 0x408})

    def test_no_line_is_wider_than_80_columns(self):
        config = configuration.load(HERE / "thermostat-yamcs.xtcs")
        with tempfile.TemporaryDirectory() as directory:
            written = Path(directory) / "tcs-ccsds.xtce"
            generated.write_mdb(config, written)
            for number, line in enumerate(written.read_text().splitlines(),
                                          1):
                self.assertLessEqual(len(line), 80, f"{number}: {line}")

    def test_the_instance_listens_where_each_source_arrives(self):
        config = configuration.load(HERE / "thermostat-yamcs.xtcs")
        self.assertEqual([(link["name"], link["port"])
                          for link in generated.data_links(config)],
                         [("commands", 17200), ("beacon", 17510),
                          ("gateway-uplink", 17600),
                          ("ambient-downlink", 17700),
                          ("gpio-in-tmass-downlink", 17702),
                          ("temp1-downlink", 17706), ("temp2-downlink", 17707),
                          ("temp3-downlink", 17708)])


class FakeYamcs:
    """A Yamcs that holds what a test gives it: each parameter's value, and
    the units its mission database gives each.

    Args:
        units: Each parameter's units, by its qualified name.
    """

    def __init__(self, units):
        self._units = units
        self.values = {}

    def units(self, name):
        """A parameter's units."""
        return self._units.get(name, [])

    def latest(self, names):
        """What has arrived of these parameters."""
        return {name: (self.values[name], 1) for name in names
                if name in self.values}


def readings_of(yamcs):
    """The example's readings, read through a Yamcs played here.

    Args:
        yamcs: The :class:`FakeYamcs`.

    Returns:
        The :class:`thermostat_yamcs.Readings`.
    """
    setup = thermostat_yamcs.Setup(options())
    return thermostat_yamcs.Readings(
        yamcs, setup.sources,
        thermostat_yamcs.feeding_of(setup.config, setup.sources))


def a_yamcs():
    """A Yamcs whose mission database is the example's: kelvin for each
    temperature, nothing for the lines sensing the heaters."""
    return FakeYamcs({f"/TcsYamcs/{name}": ["K"]
                      for name in ("temp1", "temp2", "temp3", "ambient")})


class ReadingsTest(unittest.TestCase):
    """What the program shows of what arrives."""

    def test_temperatures_are_shown_with_their_units_and_decisions_on_or_off(
            self):
        yamcs = a_yamcs()
        readings = readings_of(yamcs)
        self.assertEqual(readings.line(),
                         "ambient --, temp1 --, temp2 --, temp3 --, "
                         "gpio-in-tmass line 2 --, gpio-in-tmass line 3 --")
        yamcs.values.update({"/TcsYamcs/ambient": 291.2341,
                             "/TcsYamcs/temp1": 291.0,
                             "/TcsYamcs/gpio-in-tmass_line2": True,
                             "/TcsYamcs/gpio-in-tmass_line3": False})
        readings.read()
        self.assertEqual(readings.line(),
                         "ambient 291.234 K, temp1 291.000 K, temp2 --, "
                         "temp3 --, gpio-in-tmass line 2 on, "
                         "gpio-in-tmass line 3 off")
        self.assertFalse(readings.arrived())

    def test_a_decision_that_changes_is_said(self):
        yamcs = a_yamcs()
        readings = readings_of(yamcs)
        yamcs.values["/TcsYamcs/gpio-in-tmass_line2"] = True
        first = readings.read()
        self.assertEqual([(source.node, was, now)
                          for source, was, now in first],
                         [("gpio-in-tmass line 2", None, True)])
        self.assertEqual(readings.read(), [], "the same is no change")
        yamcs.values["/TcsYamcs/gpio-in-tmass_line2"] = False
        yamcs.values["/TcsYamcs/ambient"] = 295.2
        changed = readings.read()
        said = io.StringIO()
        with contextlib.redirect_stdout(said):
            thermostat_yamcs.say_changes(first, readings)
            thermostat_yamcs.say_changes(changed, readings)
        self.assertEqual(said.getvalue().splitlines(),
                         ["switched: gpio-in-tmass line 2 on -> off; "
                          "ambient 295.200 K, temp1 --, temp2 --, temp3 --, "
                          "gpio-in-tmass line 2 off, gpio-in-tmass line 3 "
                          "--"])

    def test_bytes_are_shown_as_hex(self):
        self.assertEqual(thermostat_yamcs.shown(
            thermostat_yamcs.Source("x", "/TcsYamcs/x", DATUM), b"\x01\xff",
            []), "01ff")


class NoBeacons:
    """A beacon link that hears nothing, at once."""

    def next_pass(self):
        """A pass in which nothing arrived."""
        return types.SimpleNamespace(remarks=[], warnings=[], at=None,
                                     arrived=Arrived.NOTHING)


class WatchTest(unittest.TestCase):
    """The program with no window."""

    def test_it_succeeds_once_every_source_has_said_something(self):
        yamcs = a_yamcs()
        readings = readings_of(yamcs)
        for source in readings.sources:
            yamcs.values[source.parameter] = (
                True if source.form == DECISION else 290.0)
        said = io.StringIO()
        with contextlib.redirect_stdout(said):
            status = thermostat_yamcs.watch_and_print(readings, NoBeacons(),
                                                      0.05, 1)
        self.assertEqual(status, 0)
        self.assertIn("read: ambient 290.000 K", said.getvalue())

    def test_it_fails_where_a_source_never_says_anything(self):
        readings = readings_of(a_yamcs())
        said = io.StringIO()
        with contextlib.redirect_stdout(io.StringIO()), \
                contextlib.redirect_stderr(said):
            status = thermostat_yamcs.watch_and_print(readings, NoBeacons(),
                                                      0.05, 1)
        self.assertEqual(status, 1)
        self.assertIn("nothing arrived from ambient", said.getvalue())


if __name__ == "__main__":
    unittest.main()
