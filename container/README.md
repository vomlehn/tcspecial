# The TCS test container

A test system, in a container, in which the TCS projects' packages are
installed and tried as an administrator would, and a checkout built,
without touching the machine it runs on: tcspecial's own, and those of
the TCS projects checked out beside it, such as tcs-xtce, tcslog and
tcslog-tools. Everything here is run from this directory, `container/`
in tcspecial's checkout.

- `bin/` -- the commands: `bin/start`, `bin/tcs-container`, and a script
  for each of its commands.
- `image/` -- what the images are built from: a Containerfile for each
  distribution, and what each puts in its image: `tcs-profile.sh`, which
  each login shell reads, and the `cfge` and `xtcee` commands.
- `Makefile` -- `make container` builds both images, `make kubuntu` and
  `make fedora` one each, and the `container-` targets below use the
  container; `make help` lists them.
- `README.html` -- this page as HTML, made from `README.md` with
  `make html`, which tcspecial's manual links.

## What it is

The scripts in `bin/` keep a test system in a container, to install a
project's packages into and remove them from as an administrator would,
and to build a checkout in, without touching the machine it runs on.
It uses Podman, rootless (`sudo apt install podman` on Ubuntu), and its
images are built from `image/Containerfile.kubuntu`, for the
.debs -- Ubuntu with KDE Plasma's libraries, theme and fonts, as on a
Kubuntu machine -- and `image/Containerfile.fedora`, for the
RPMs. Each holds the basic command set, Python, git and make, systemd,
which runs as the container's first process so that a package's scripts
and units behave as on a real system, and a user, `tcs`, with sudo.
Fedora's own image tells dnf to leave out every package's documentation;
this one does not, so that a package's documents are installed and
tested as on a Fedora machine.

They are used from a terminal, with no desktop session of their own, but a
program with a window shows it on your desktop. `tcs` has your uid, there
as on the host, and `fresh` gives the container your Wayland and X11
sockets, your X authority and your GPU, so `bin/tcs-container-shell
tcs-cfge` opens tcs-cfge's window beside your others. A display gone since,
as after logging out and in again, needs `bin/tcs-container-fresh`. Xvfb
is there too, for a program to open its window where nobody sees it. What
a package needs it brings with it, and what building a project needs that
project's `make install-tools` installs.

The quickest way in: build the packages of whatever you are testing, then
have the newest of them installed in a container just made:

```sh
make -C .. packages
bin/start                               # the .debs, in kubuntu
bin/start -f                            # the RPMs, in fedora
```

`bin/start [-f | --fedora] [DIR...]` takes the `.deb` files, or with
`-f` or `--fedora` the `.rpm` files, from `build/packages` of each TCS
checkout, this tcspecial and those beside it (`../../tcs*`), or of each
DIR given. From each, it takes only the files at the version of its
newest one, so that an older build left beside them, or a package since
renamed, is not installed. It starts the container afresh, installs them all in one go,
and opens a new terminal window with a login shell in the container, as
`tcs`, to try them in. The terminal is `$TERMINAL`, or the first there
of `x-terminal-emulator` (Konsole on Kubuntu), `konsole`,
`gnome-terminal` and `xterm`; with no display, or no terminal,
`bin/start` says so and starts nothing.

`bin/start` then waits until the container stops. Leaving the window's
shell, or closing the window, stops it, with everything in it, other
shells such as `bin/tcs-container-shell` gives too, and so does Ctrl+C in
the terminal `bin/start` was run from. The commands below are run from
another terminal on the host while it waits.

The commands, each a script of its own in `bin/`, `tcs-container-` and
its name, such as `bin/tcs-container-fresh`:

```sh
fresh             # throw the container away, start anew
install FILE...   # install .deb or .rpm files
installed         # the TCS packages it has
shell             # a login shell in it, as tcs
shell tcslog-dump # or a command
shell tcs-cfge    # one with a window, on your desktop
shell --root CMD  # a command as root
remove PACKAGE... # remove packages, configuration too
copy DIR...       # copy checkouts in, without their builds
example [NAME]    # run a tcspecial example, or list them
wait              # wait until it stops
stop              # stop it, keeping what was done in it
build             # build its image
destroy [--image] # remove it, and with --image its image
```

Each takes `--distro fedora` first for the Fedora container, kubuntu being
the default:

```sh
bin/tcs-container-copy ..
bin/tcs-container-shell make -C src/tcspecial install-tools
bin/tcs-container-fresh --distro fedora
bin/tcs-container-installed --distro fedora
```

`fresh` builds the image when there is none, and every command that needs
the container starts it. A package installed again at the same version is
reinstalled, so that a package rebuilt without a new version is the one
tested.

`make container-install` builds the packages of the project checked out
in `PROJECT`, this tcspecial checkout unless it names another, and
installs them in the container; `make container-remove PACKAGES=...`
removes them, and `make container-installed`, `make container-fresh` and
`make container-shell` do what the commands above do. `PACKAGES` chooses
packages to install, such as only the one that flies, and
`DISTRO=fedora` the RPMs. `make container` builds both images, and
`make kubuntu` and `make fedora` one each, as `build` does; `make check`
runs shellcheck on the scripts:

```sh
make container-fresh
make container-install PACKAGES=tcspecial
make container-shell
make container-remove PACKAGES=tcspecial
```

## Running tcspecial's examples

Every example tcspecial has is in its `tcspecial-examples` package, with
the programs it runs and `tcsx`, which runs one, so `bin/start` installs
them all with the rest of tcspecial's packages. `bin/tcs-container-example`
runs `tcsx` in the container, each example's window on your desktop. With
no name it lists them, saying what any needs beyond the packages:

```sh
make -C .. packages
bin/start                               # tcspecial's packages, installed
```

and, in another terminal on the host, while `bin/start` waits:

```sh
bin/tcs-container-example               # list the examples
bin/tcs-container-example thermostat    # run one, until its window closes
```

The window `bin/start` opens says how to run them there: `tcsx` lists
them, and `tcsx NAME` runs one. There, as from `bin/tcs-container-example`,
`tcsx` is given `--no-memlock`: a rootless container's limit on locked
memory is the host's, too small for tcspecial to lock its memory into RAM,
as it does by default. Arguments after the name go to the example's own
ground station, `-v` making it say what it is doing. Yamcs 5.14.0, which
the examples through Yamcs start, is in each image, in `/opt/yamcs`, with
the Java it runs on, and the login shells' `YAMCS_HOME` says so:

```sh
bin/tcs-container-example ping-yamcs
```

`can`, `gpio` and `iio` drive the kernel's dummy devices, which a rootless
container cannot make, so they run only on the host. The logs the
examples write are in the container's `/tmp/telemetry`, made afresh for
each run. A run lasts until its window is closed or Ctrl+C, and one left
running, as by a terminal closed under it, is stopped by the next. `make
container-example EXAMPLE=thermostat` does the same, with `HIDDEN=1` and
`ARGS=` for the options.

`--hidden` is for testing: it draws the example's window with Xvfb, in
the container, where nobody sees it, so that a test, or a check run while
you work, opens no window on your desktop. Such a run has no window to
close, so it is given a time to last, and what it did is read from its
output and its log afterwards:

```sh
timeout 30 bin/tcs-container-example --hidden battery > battery.out 2>&1
grep -c ERROR battery.out                       # 0: nothing went wrong
bin/tcs-container-shell ls /tmp/telemetry       # the log it wrote
```

The next run stops what the timeout left running in the container.

## Editing configurations

Each image has `cfge`, which opens a tcspecial configuration in
tcspecial's configuration editor, `tcs-cfge`: a file, which need not exist
yet, the editor beginning a new one there, or with `-e` or `--example`
one of tcspecial's examples by name, its `.xtcs` file with its simulator
file. The editor writes every edit as it is made, so the first `-e`
copies the installed examples to `~/tcspecial-examples`, and every `-e`
edits the copy there. Remove the copy to start again from what is
installed.

```sh
cfge vehicle.ytcs               # opens vehicle.ytcs, or begins it
cfge -e                         # lists the examples
cfge -e thermostat              # edits thermostat.xtcs, and its sim file
```

An example has both spellings, `.xtcs` and `.ytcs`, and the editor writes
every edit to both, so editing one keeps the other the same.

## Editing XTCE

Each image has `xtcee`, which opens a file, or one of tcs-xtce's examples
by name, in tcs-xtce's editor, `tcs-xtcee`, once the tcs-xtce package is
installed, as `bin/start` installs it. An example of several documents,
such as `split-mission`, opens all of them together, as one mission. The
window `bin/start` opens says how to use it:

```sh
xtcee FILE                      # opens FILE
xtcee -e                        # lists the examples
xtcee -e housekeeping           # opens housekeeping.xtce
xtcee --example split-mission   # opens its three documents
```

## tcs-container

Every command above is one of `bin/tcs-container`'s; the scripts named for
them run it:

```sh
bin/tcs-container [--distro kubuntu|fedora] COMMAND [ARGUMENT...]
```

`TCS_CONTAINER_DISTRO` names the distribution too, when `--distro` does
not, and `PODMAN` another program to run in Podman's place. The container
is `tcs-test-<distro>`, made from the image `localhost/tcs-test-<distro>`,
which `build` makes from `image/Containerfile.<distro>` with the user
`tcs` given your uid and gid. `fresh` runs it with `--systemd=always` and
`--userns=keep-id`, so that systemd is its first process and `tcs` is you,
and gives it your display, as above. `install` copies the files in and
hands them to `apt-get` or `dnf`, which fetch what they depend on: the
packages not installed at the version given in one transaction, new ones
and upgrades together, so that one may require another given with it, and
those already installed at that version reinstalled, so that a package
rebuilt at the same version is the one tested.
