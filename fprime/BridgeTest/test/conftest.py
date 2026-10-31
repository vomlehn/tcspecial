"""BridgeTest, started once F´'s test API is listening.

Started here rather than by ``bridge-test`` so that nothing the bridge says
as it starts -- its first beacon, its match -- is said before there is
anyone to hear it. ``bridge-test`` says where everything is, in the
environment:

    TCS_BRIDGE_DEPLOYMENT  the BridgeTest program
    TCS_CONFIGURATION      the tcspecial configuration it opens
    TCS_GDS_PORT           where F´'s ground system listens for it
    TCS_RUN                the directory its log is written in
"""

import os
import subprocess
from pathlib import Path

import pytest

# How long BridgeTest has to stop once told to, in seconds, before it is
# killed.
STOPPING = 10


@pytest.fixture(scope="session", autouse=True)
def bridge(fprime_test_api_session):
    """Runs BridgeTest for the whole session, from once the test API has
    connected to F´'s ground system to when the last test is done.

    Args:
        fprime_test_api_session: F´'s test API, connected.

    Yields:
        The running BridgeTest.
    """
    run = Path(os.environ["TCS_RUN"])
    with open(run / "BridgeTest.log", "wb") as log:
        program = subprocess.Popen(
            [
                os.environ["TCS_BRIDGE_DEPLOYMENT"],
                "-a",
                "127.0.0.1",
                "-p",
                os.environ["TCS_GDS_PORT"],
                "-c",
                os.environ["TCS_CONFIGURATION"],
            ],
            cwd=run,
            stdout=log,
            stderr=subprocess.STDOUT,
        )
        try:
            yield program
        finally:
            # Told to stop as a person stops it, and held to stopping
            # cleanly: a bridge that crashed on the way down, or would not
            # go, is a failed run however its tests went.
            program.terminate()
            try:
                status = program.wait(timeout=STOPPING)
            except subprocess.TimeoutExpired:
                program.kill()
                program.wait()
                pytest.fail(f"BridgeTest did not stop in {STOPPING} s")
            assert status == 0, f"BridgeTest stopped with status {status}"
