"""The programs a ground station starts, each killed when it exits.

A ground station on a desk often starts what it talks to: tcspecial on its
configuration, tcssim simulating the payloads, Yamcs between the two.
Each is started here told to die with the station -- however it exits, the
kernel being asked to see to it where the station is killed or crashes, so
that nothing is left beaconing, or holding an example's ports, with no
station there. The ping example's ground stations and the thermostat's
through Yamcs share this.

Linux only, as tcspecial is: the kernel is asked with ``prctl``.
"""

import ctypes
import os
import signal
import subprocess
import sys
import threading

from tcslibgs.configuration import find_built

#: prctl's option for the signal a process is sent when its parent dies.
PR_SET_PDEATHSIG = 1


def die_with_this_program(parent):
    """In the child, before its program runs: be killed when ``parent``
    dies.

    A program that exits normally kills its children itself; one that is
    killed or crashes runs nothing at all, and the children would go on with
    nobody to stop them. So the kernel is asked to send SIGKILL when this
    program goes -- and if it has gone already, between the fork and here,
    the child goes now rather than never being told.

    Args:
        parent: This program's process id, taken before the fork.
    """
    libc = ctypes.CDLL(None, use_errno=True)
    if libc.prctl(PR_SET_PDEATHSIG, signal.SIGKILL, 0, 0, 0) != 0:
        os._exit(127)
    if os.getppid() != parent:
        os._exit(1)


class Child:
    """A program started by this one and killed when it exits.

    Run itself and not through ``cargo run``: the kill is ``kill()``, and
    killing cargo would leave the program it started running, holding the
    example's ports, with nothing left that knows it is there.

    Args:
        program: The program to run.
        args:    Its arguments.
        name:    What to call it when it stops by itself.
        verbose: Whether to say on stderr that it stopped by itself.
        env:     Its environment, or ``None`` for this program's.
    """

    def __init__(self, program, args, name=None, verbose=False, env=None):
        self.program = program
        self.args = list(args)
        self.name = name or os.path.basename(str(program))
        self.verbose = verbose
        self.env = env
        self.process = None
        self._stopping = False

    def start(self):
        """Start it, told to die with this program, and say if it stops by
        itself."""
        parent = os.getpid()
        self._stopping = False
        self.process = subprocess.Popen(
            [self.program, *self.args], env=self.env,
            preexec_fn=lambda: die_with_this_program(parent))
        threading.Thread(target=self._watch, args=(self.process,),
                         daemon=True).start()

    def _watch(self, process):
        """Wait for it to exit, and say so if nobody stopped it.

        Args:
            process: The process to wait for.
        """
        code = process.wait()
        if not self._stopping and self.verbose:
            print(f"{self.name} stopped by itself: exit status {code}",
                  file=sys.stderr, flush=True)

    def stopped(self):
        """Whether it was started and has exited.

        Returns:
            True once its process has exited, by itself or killed.
        """
        return self.process is not None and self.process.poll() is not None

    def stop(self):
        """Kill it, and wait for it to be gone, so that it is reaped and its
        ports are free."""
        if self.process is None:
            return
        self._stopping = True
        if self.process.poll() is None:
            self.process.kill()
        self.process.wait()
        self.process = None


def built(program, hint):
    """A program of this tree's build.

    Args:
        program: Its name: ``tcspecial``, ``tcssim``.
        hint:    What to do where it is missing, for the complaint.

    Returns:
        Its path.

    Raises:
        ValueError: It is not built or on the PATH.
    """
    found = find_built(program)
    if found is None:
        raise ValueError(f"{program} is not built or on the PATH: build it "
                         f"with `cargo build --bin {program}`, or {hint}")
    return found


class Vehicle(Child):
    """tcspecial, started by this program and killed when it exits.

    Args:
        program: The tcspecial program to run.
        args:    Its arguments.
        verbose: Whether to say on stderr that it stopped by itself.
    """

    def __init__(self, program, args, verbose=False):
        super().__init__(program, args, "tcspecial", verbose)

    @staticmethod
    def for_configuration(path, program=None, verbose=False, simulate=None):
        """tcspecial on this configuration: the one named, or the build's.

        Told ``--quiet`` unless ``verbose``, so that the console is this
        program's: tcspecial's startup summary is for somebody watching the
        vehicle start, and what stops it starting it says either way.

        Args:
            path:     The configuration file.
            program:  The tcspecial to run, or ``None`` for the build's.
            verbose:  Whether to let tcspecial print its startup summary.
            simulate: The ``--simulate`` mode to give it, or ``None`` for
                      none, which is the flight setting.

        Returns:
            The :class:`Vehicle`, not yet started.

        Raises:
            ValueError: No program was named and none is built or on the
                        PATH.
        """
        program = program or built("tcspecial", "say --no-vehicle")
        args = [] if verbose else ["--quiet"]
        if simulate is not None:
            args.append(f"--simulate={simulate}")
        return Vehicle(program, args + [str(path)], verbose)
