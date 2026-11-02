"""What a node sends, as the configuration says it, and the Yamcs parameter
type it is given for it.

The Python spelling of ``tcslibgs::graph::fprime::Form::of``, which
``examples/thermostat-yamcs/tests/agrees.rs`` holds to the Rust for every
node of every example; and the typed parameters ``tcslibgs.yamcs`` gives a
node whose data no interface document lays out.
"""

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from tcslibgs import configuration, interface  # noqa: E402
from tcslibgs import yamcs as generated  # noqa: E402
from tcslibgs.form import (  # noqa: E402
    DATUM, DECISION, UNITS, Form, form_of, unit_of_symbol)

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
        payload-in: true
vehicle:
  - type compute:
      - compute: c
        {c}
        format: "i16_be"
        offset: "0"
        scale: "1"
        payload-out:
          - target: g
          - target: v
            input: in0
      - compute: d
        {d}
        format: "i16_be"
        offset: "0"
        scale: "1"
        payload-out:
          - target: g
          - target: v
            input: in1
      - compute: p
        function: "process"
        executable: "/bin/cat"
        unit: "W"
        payload-out:
          - target: g
      - compute: t
        function: "hysteresis"
        half_band: "1"
        setpoint: "10"
        payload-out:
          - target: g
      - compute: v
        algorithm: "median"
        function: "vote"
        inputs: "in0,in1"
        payload-out:
          - target: g
          - target: t
payloads:
  - type network:
      - payload: s
        address: "127.0.0.1"
        port: "9401"
        protocol: "udp"
        payload-out:
          - target: c
          - target: g
"""

#: How a conversion to kelvin, and one to a unit, are written.
TO_KELVIN = 'function: "to_kelvin"'


def to_value(symbol):
    """How a conversion to a unit is written.

    Args:
        symbol: The unit, as a file writes it.

    Returns:
        The node's ``function`` and ``unit`` lines.
    """
    return f'function: "to_value"\n        unit: "{symbol}"'


def vehicle(directory, c=TO_KELVIN, d=TO_KELVIN):
    """Writes :data:`VEHICLE` and reads it.

    Args:
        directory: Where to write it.
        c:         How "c" is written: its function, and unit if any.
        d:         How "d" is written.

    Returns:
        The :class:`configuration.Configuration`.
    """
    path = Path(directory) / "vehicle.ytcs"
    path.write_text(VEHICLE.format(interface=interface.of_this_tree().hex(),
                                   c=c, d=d))
    return configuration.load(path)


class FormTest(unittest.TestCase):
    def test_each_node_is_what_its_function_makes(self):
        with tempfile.TemporaryDirectory() as directory:
            config = vehicle(directory)
            kelvin = Form("Value", UNITS[0])
            self.assertEqual(
                {node.name: form_of(config, node.name)
                 for node in config.nodes},
                {"c": kelvin, "d": kelvin, "v": kelvin, "t": DECISION,
                 "p": Form("Value", unit_of_symbol("W", "p")), "g": DATUM,
                 "s": DATUM})
            self.assertEqual(form_of(config, "nobody"), DATUM)
            self.assertEqual(str(kelvin), "Value(Kelvin)")
            self.assertEqual(str(DECISION), "Decision")

    def test_a_vote_is_in_its_inputs_unit(self):
        with tempfile.TemporaryDirectory() as directory:
            config = vehicle(directory, to_value("m/s"), to_value("m/s"))
            self.assertEqual(str(form_of(config, "v")),
                             "Value(MetrePerSecond)")

    def test_what_tcsverify_refuses_is_not_read_as_anything(self):
        with tempfile.TemporaryDirectory() as directory:
            config = vehicle(directory, to_value("V"))
            with self.assertRaisesRegex(ValueError, "2 units"):
                form_of(config, "v")
            config = vehicle(directory, to_value("furlongs"))
            with self.assertRaisesRegex(ValueError, "not a unit"):
                form_of(config, "c")
            config = vehicle(directory, 'function: "to_value"')
            with self.assertRaisesRegex(ValueError, "unit is not set"):
                form_of(config, "c")


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
            types = self.types(vehicle(directory, to_value("V"),
                                       to_value("V")))
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
        self.assertEqual(types["s"].get("name"), generated.BODY_TYPE)

    def test_a_ratio_has_no_unit(self):
        with tempfile.TemporaryDirectory() as directory:
            types = self.types(vehicle(directory, to_value("ratio"),
                                       to_value("ratio")))
        self.assertEqual(types["c"].get("name"), "RatioValueType")
        self.assertIsNone(types["c"].find(
            f"./{{{generated.NAMESPACE}}}UnitSet"))

    def test_a_vote_between_units_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            config = vehicle(directory, to_value("V"))
            with self.assertRaisesRegex(ValueError, "2 units"):
                generated.mdb(config)


if __name__ == "__main__":
    unittest.main()
