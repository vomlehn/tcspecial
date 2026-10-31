"""The interface a configuration names, and the fixed commands' layouts.

A configuration's ``tcspecial`` section names, as ``interface``, the digest
of the ``link.xml`` its commands are laid out by, and a ground station in
Python refuses one that is not this tree's as the Rust does. The three fixed
commands are in ``fixed.xml``, apart from the interface, and are packed by
the code tcs-xtce generates from it: these hold that code to the layouts the
design's table gives.
"""

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from tcslibgs import configuration, fixed, fixed_packet  # noqa: E402
from tcslibgs import interface  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]


def section(said):
    """A configuration whose ``tcspecial`` section says ``said``, in a file
    of its own.

    Args:
        said: What goes in the section, as XML.

    Returns:
        A pair: the temporary directory holding the file, which removes it
        when cleaned up, and the file's path.
    """
    directory = tempfile.TemporaryDirectory()
    path = Path(directory.name) / "vehicle.xml"
    path.write_text(
        f"<configuration><tcspecial>{said}</tcspecial></configuration>"
    )
    return directory, path


class InterfaceTest(unittest.TestCase):
    def test_the_interface_is_link_xml_alone(self):
        # Not fixed.xml beside it: no change to the interface may touch the
        # fixed commands.
        self.assertEqual(
            interface.of_this_tree(),
            interface.of_files([interface.DOCUMENTS / "link.xml"]),
        )
        self.assertNotEqual(
            interface.of_this_tree(),
            interface.of_files(sorted(interface.DOCUMENTS.glob("*.xml"))),
        )

    def test_this_trees_interface_is_what_the_examples_name(self):
        named = configuration.load(ROOT / "examples" / "ping" / "ping.xml")
        self.assertEqual(
            named.tcspecial["interface"], interface.of_this_tree().hex()
        )

    def test_a_configuration_of_another_interface_is_refused(self):
        ours = interface.of_this_tree().hex()
        other = "0123456789abcdef0123456789abcdef"
        for said, words in [
            ("<gateway>radio</gateway>", ["interface is not stated", ours]),
            (f"<interface>{ours.upper()}</interface>",
             ["not an interface digest", "--interface-digest"]),
            (f"<interface>{other}</interface>", [other, ours, "interface:"]),
        ]:
            directory, path = section(said)
            with directory:
                with self.assertRaises(ValueError, msg=said) as refused:
                    configuration.load(path)
                for word in words:
                    self.assertIn(word, str(refused.exception), said)
                self.assertIn(str(path), str(refused.exception))
        directory, path = section(f"<interface>{ours}</interface>")
        with directory:
            configuration.load(path)


class FixedCommandsTest(unittest.TestCase):
    def test_connect_and_its_answer_are_the_tables_layouts(self):
        checksum = bytes(range(16))
        packed = fixed.connect(7, checksum)
        self.assertEqual(packed, bytes([4, 0, 0, 0, 7]) + checksum)

        answer = (bytes([4, 0, 0, 0, 7, 0]) + bytes(6) + bytes([1, 2])
                  + checksum + bytes([1, 2, 3, 0, 0, 0, 40]))
        read = fixed.answer(answer)
        self.assertIsInstance(read, fixed_packet.ConnectAnswer)
        self.assertEqual(read.sequence, 7)
        self.assertEqual(read.timestamp, 0x0102)
        self.assertEqual(bytes(read.running), checksum)
        self.assertEqual((read.major, read.minor, read.patch), (1, 2, 3))
        self.assertEqual(read.newest_taken, 40)

    def test_an_answer_of_the_wrong_length_is_refused(self):
        answer = (bytes([4, 0, 0, 0, 7, 0]) + bytes(8) + bytes(16)
                  + bytes([1, 2, 3, 0, 0, 0, 40]))
        self.assertIsNotNone(fixed.answer(answer))
        for wrong in (answer[:-1], answer + b"\0", b"", b"\x30" + answer[1:]):
            self.assertIsNone(fixed.answer(wrong), wrong.hex())


if __name__ == "__main__":
    unittest.main()
