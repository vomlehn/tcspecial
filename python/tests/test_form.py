"""What a node sends the ground, as the configuration's own XTCE says it,
and the Yamcs parameter type it is given for it.

The Python spelling of ``tcslibgs::graph::fprime::Form::of``, which
``examples/thermostat-yamcs/tests/agrees.rs`` holds to the Rust for every
node of every example that reaches the ground; and the typed parameters
``tcslibgs.yamcs`` copies from the configuration's own XTCE for a node whose
data no interface document lays out.
"""

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from tcslibgs import configuration, interface  # noqa: E402
from tests import loaded  # noqa: E402
from tcslibgs import yamcs as generated  # noqa: E402
from tcslibgs.form import (  # noqa: E402
    DATUM, DECISION, UNITS, Form, form_of, unit_of_type_name)

#: A vehicle that answers to Yamcs: a sensor "s" converted by "c" (to
#: kelvin, unless the test says), a second conversion "d", a vote "v"
#: between the two, a thermostat "t" on the vote, and a program "p", each
#: drawn into the gateway node "g".
VEHICLE = """\
tcspecial:
  gateway: yamcs
  interface: "{interface}"
  beacon: {{ apid: "0x100", recv: "127.0.0.1:9100", send: "0.0.0.0:9101" }}
  tcs_cmd: {{ apid: "0x101", recv: "127.0.0.1:4000", send: "127.0.0.1:0" }}
gateways:
  - type gateway_yamcs:
      - gateway: g
        apid_base: "0x200"
        ground_base: "127.0.0.1:9000"
        world-in: [c, d, p, t, v, s]
vehicle:
  - type compute:
      - compute: c
        {c}
        world-in: [s]
        type-in: "i16"
        offset: "0"
        scale: "1"
        world-out:
          - g.c
          - v.in0
      - compute: d
        {d}
        type-in: "i16"
        offset: "0"
        scale: "1"
        world-out:
          - g.d
          - v.in1
      - compute: p
        function: "process"
        executable: "/bin/cat"
        type-in: "kelvin"
        type-out: "watt"
        world-out:
          - g.p
      - compute: t
        function: "thermostat"
        half_band: "1"
        setpoint: "10"
        type-in: "kelvin"
        type-out: "boolean"
        world-in: [v]
        world-out:
          - g.t
      - compute: v
        algorithm: "median"
        carries: "{carries}"
        function: "vote"
        world-in: [in0, in1]
        world-out:
          - g.v
          - t.v
payloads:
  - type network:
      - payload: s
        address: "127.0.0.1"
        port: "9401"
        protocol: "udp"
        world-out:
          - c.s
          - g.s
"""

#: How a conversion to kelvin, and one to a unit, are written.
TO_KELVIN = 'function: "to_kelvin"'


def to_value(kind):
    """How a conversion to a unit is written.

    Args:
        kind: The value type it sends: ``volt``, ``metre_per_second``.

    Returns:
        The node's ``function`` and ``type-out`` lines.
    """
    return f'function: "to_value"\n        type-out: "{kind}"'


def vehicle(directory, c=TO_KELVIN, d=TO_KELVIN, carries="kelvin"):
    """Writes :data:`VEHICLE` and reads it.

    Args:
        directory: Where to write it.
        c:         How "c" is written: its function, and unit if any.
        d:         How "d" is written.
        carries:   The value type the vote "v" carries.

    Returns:
        The :class:`configuration.Configuration`.
    """
    path = Path(directory) / "vehicle.ytcs"
    path.write_text(VEHICLE.format(interface=interface.of_this_tree().hex(),
                                   c=c, d=d, carries=carries))
    return loaded(path)


class FormTest(unittest.TestCase):
    def test_each_node_is_what_its_function_makes(self):
        with tempfile.TemporaryDirectory() as directory:
            config = vehicle(directory)
            kelvin = Form("Value", UNITS[0])
            self.assertEqual(
                {node.name: form_of(config, node.name)
                 for node in config.nodes},
                {"c": kelvin, "d": kelvin, "v": kelvin, "t": DECISION,
                 "p": Form("Value", unit_of_type_name("watt", "p")), "g": DATUM,
                 "s": DATUM})
            self.assertEqual(form_of(config, "nobody"), DATUM)
            self.assertEqual(str(kelvin), "Value(Kelvin)")
            self.assertEqual(str(DECISION), "Decision")

    def test_a_conversion_is_in_the_type_it_states(self):
        with tempfile.TemporaryDirectory() as directory:
            config = vehicle(directory, to_value("metre_per_second"))
            self.assertEqual(str(form_of(config, "c")),
                             "Value(MetrePerSecond)")

    def test_what_tcsverify_refuses_has_no_own_xtce_to_read(self):
        # A type that is none, and none stated: tcsverify writes no own
        # XTCE for either, and what each node sends is not guessed at.
        for c in (to_value("furlongs"), 'function: "to_value"'):
            with tempfile.TemporaryDirectory() as directory:
                config = vehicle(directory, c)
                with self.assertRaisesRegex(ValueError, "not there"):
                    form_of(config, "c")

    def test_a_node_that_does_not_reach_the_ground_is_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            self.assertEqual(form_of(vehicle(directory), "g"), DATUM)


class TypedParameterTest(unittest.TestCase):
    """The mission database gives each source the type its data are."""

    def types(self, config):
        """Each source's parameter type, and the type's element.

        Args:
            config: The configuration.

        Returns:
            Each parameter's type's element, by the parameter's name.
        """
        root = generated.mdb(config).getroot()
        x = {"x": generated.NAMESPACE}
        kinds = {kind.get("name"): kind for kind in root.iterfind(
            ".//x:ParameterTypeSet/*", x)}
        return {parameter.get("name"): kinds[parameter.get(
            "parameterTypeRef")] for parameter in root.iterfind(
            ".//x:ParameterSet/x:Parameter", x)}

    def local(self, element):
        """An element's name, without the namespace.

        Args:
            element: The element.

        Returns:
            Its local name.
        """
        return element.tag.split("}", 1)[1]

    def test_values_decisions_and_bytes_each_have_their_type(self):
        with tempfile.TemporaryDirectory() as directory:
            types = self.types(vehicle(directory, to_value("volt"),
                                       to_value("volt"), "volt"))
        x = {"x": generated.NAMESPACE}
        for name, unit, coefficient in (("c", "V", 1e-6), ("v", "V", 1e-6),
                                        ("p", "W", 1e-3)):
            kind = types[name]
            self.assertEqual(self.local(kind), "FloatParameterType", name)
            self.assertEqual(kind.find("./x:UnitSet/x:Unit", x).text, unit)
            term = kind.find("./x:IntegerDataEncoding/x:DefaultCalibrator"
                             "/x:PolynomialCalibrator/x:Term", x)
            self.assertEqual(float(term.get("coefficient")), coefficient)
        self.assertEqual(self.local(types["t"]), "BooleanParameterType")
        # A sensor that states no type sends bytes, after the count of them
        # it is sent the ground after, as XTCE encodes it.
        self.assertEqual(types["s"].get("name"), "bytes")
        self.assertEqual(self.local(types["s"]), "BinaryParameterType")

    def test_a_value_type_is_types_xtces(self):
        with tempfile.TemporaryDirectory() as directory:
            types = self.types(vehicle(directory, to_value("ratio"),
                                       to_value("ratio"), "ratio"))
        x = {"x": generated.NAMESPACE}
        self.assertEqual(types["c"].get("name"), "ratio")
        self.assertEqual(types["c"].get("sizeInBits"), "64")
        self.assertEqual(types["c"].find("./x:UnitSet/x:Unit", x).text,
                         "ratio")


if __name__ == "__main__":
    unittest.main()
