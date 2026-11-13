===================
Writing Node Types
===================
.. note::
   Links between nodes carry types: see "Typed links" in the design. All
   of it that a node type declares or relies on is built. What is still to
   come, the commands for the attributes command bindings set, changes
   nothing here. Where the code and the design differ, the code is what is
   described here.

.. contents:: Table of Contents
   :depth: 2
   :local:

Introduction
============
A node is anything in a configuration's graph: a payload reached over an
interface, a computation aboard, a log, an app, a gateway. Each has a type,
the word after ``type`` in the configuration, and this guide says how to add
a new one: which files it lives in, what it must declare, which lists it must
be added to, and how to know it is finished.

It is written for someone changing tcspecial itself. A user who wants a new
kind of payload without changing tcspecial should first look at the app node
types in the manual, which run a program of one's own as a node.

The order of the qualities that decides tcspecial's designs decides a node
type's too: **reliability**, then **ease of use**, then **maintainability**,
then **flexibility**. A configuration the new type refuses when it is loaded
is better than one it fails on in flight, and a type that does one thing
with fixed, checked attributes is better than one that adapts.

The Kinds of Node Type
======================
Which kind a new type is decides how much there is to do.

Payload types
    A device the vehicle reaches over an interface: ``i2c``, ``spi``,
    ``can``, ``network``, ``gpio_in`` and so on. These are the most work,
    since the flight software opens the device, the simulator plays it, and
    the vehicle judges whether what it found is simulated, a dummy or real.
    Most of this guide is about them.

Vehicle types
    Nodes that run aboard and reach no device: ``log``, ``ping``, the
    ``tee-*`` and ``null-*`` types. Fewer places, but each is matched by
    name, so the compiler does not point at the ones that were missed.

Compute functions
    Not a node type. ``compute`` is the type and its ``function`` chooses
    what it does: ``vote``, ``thermostat``, ``to_kelvin``. Adding a
    function is the smallest of these jobs: see `Adding a Compute
    Function`_.

Gateway types
    The side of the graph that commands the vehicle and takes its data:
    ``gateway_radio``, ``gateway_fprime``, ``gateway_cfs``,
    ``gateway_yamcs``. A gateway is a network endpoint, opened as a
    ``network`` payload is, so it has no node module of its own in any
    crate.

App types
    A program of one's own, run as a node: ``app_payload_veh``,
    ``app_gateway_veh``, ``app_all_veh``, ``app_pl`` and ``app_all_pl``.
    They are named stream-then-side, and a new one follows that rule.

Three Modules of One Name
=========================
A payload type is three modules with the same name, one in each of three
crates:

``tcslib_protocol::node``
    Its attributes, and every rule about what a configuration may say about it.
    tcslib-protocol is shared by the ground and the vehicle, so a rule written
    here is the one tcspecial, tcs-sim, tcs-moc, tcs-gui and tcs-verify all apply,
    and they all report it alike. A rule about what a configuration may say
    belongs here and nowhere else.

``tcspecial::node``
    The flight side: opening the device, reading and writing it.

``tcs_sim::node``
    The simulator's simulated device, which plays the payload on a desk.

Payloads are under ``node/payload/<type>/mod.rs`` in each crate, and vehicle
types under ``node/vehicle/``. A type that is another type's device under a
name of its own, as ``i2c_tmass`` is an I\ :sup:`2`\ C device, may have
modules that say so and little else, reusing the other type's code.

Terminals and What They Carry
=============================
Every node has up to four terminals, named for the way data travels:
``world-in`` and ``world-out`` carry a payload's readings towards the ground,
and ``moc-in`` and ``moc-out`` carry what the ground sends back. A new type says
which of the four it has, in ``terminals_of`` in
``tcslib-protocol/src/graph/terminals.rs``, by returning one of the
``Terminals`` constants there or a new one. A type whose terminals are its own
business, as a computation's named inputs are, returns ``None``.

Each terminal carries one type, and both ends of every link state it: a link
whose ends disagree is refused as the configuration is loaded, saying which
attribute to change. Nothing is inferred and nothing is converted between nodes.
The types are stated once, in ``tcslib-protocol/xtce/types.xtce``, and named in
``tcslib-protocol/src/link_type.rs``: the integers ``u8`` to ``i64``,
``boolean``, the quantities (``kelvin``, ``volt``, ``ampere``, ``watt``,
``metre``, ``metre_per_second``, ``newton``, ``kilogram_per_second``,
``ratio``), ``string``, ``bytes``, and structures of up to sixteen named fields,
``"x: i16, y: i16"``. A structure's fields are of fixed width, so a ``string``
or ``bytes`` field in one is refused. A new node type adds no type there.

What a node type carries is decided in one function, ``carries`` in
``tcslib-protocol/src/graph/link_types.rs``, which takes a node and a terminal
and says what that terminal carries. There are four ways a type can answer:

Stated by the configuration
    A device states what it sends as ``world-out-type`` and what it takes
    as ``moc-in-type``, each ``bytes`` when it is left out, and ``format``
    only where the part's bytes are not the type's own layout -- ``i16_le``,
    or ``u24_be`` read as a ``u32``. An app states ``world-in-type``,
    ``world-out-type``, ``moc-in-type`` and ``moc-out-type``, by the
    terminals its kind has. A computation states ``type-in`` and
    ``type-out``: a thermostat's ``type-out``, its decision, is a
    ``boolean`` or a whole number such as ``u8``. A vote, which passes one
    type through, states ``carries``, as an optional ``carries`` on a tee
    does.

Fixed by the type
    ``ping`` carries ``string``, and ``to_kelvin`` sends ``kelvin``. A GPIO
    node carries the narrowest unsigned whole number, ``u8`` to ``u64``,
    with a bit for each of its ``lines``, bit 0 the first listed: a
    ``gpio_out`` or ``gpio_inout`` is driven with that number, a bit
    beyond its lines refused, and a ``gpio_inout`` reports what it was last
    driven with and a ``gpio_in`` its lines' levels, as that number, every
    ``state_interval``, which a ``gpio_in`` must state, and a ``gpio_in``
    also whenever a datum arrives at its ``world-in``. Since the width
    comes from the lines, a GPIO node may not state ``read_size``.

Any type
    A gateway, a log and the null types take whatever arrives, since they
    pass it on or keep it without looking at it.

The default
    A payload with no branch of its own is a device, and gets the device
    rule: ``world-out-type``, ``moc-in-type`` and ``format``. A new device
    needs nothing in ``carries``.

A vehicle type with no branch of its own also falls through to the device
rule, which is almost never right and is not refused, so a new vehicle type
must have its branch. The attributes a type's terminals are stated by are in
``TYPE_ATTRIBUTES`` in the same file, and its ``problems`` is what the
configuration check calls to hold every link to the rule.

What a Node Hands On
====================
Between two nodes a datum is its type's own encoding, and only that: a whole
number as wide as its type, most significant byte first; a boolean as one
byte, ``1`` or ``0``; a value as its four bytes; a structure as its fields
one after another; a string or bytes as their contents alone. What a node
produces at a terminal is that encoding of the type the terminal carries,
with nothing before it or after it.

Every datum delivered to a node is checked against the type the node takes
there before the node is given it -- its width, a boolean's being ``1`` or
``0``, a string's being UTF-8, and no datum longer than 4096 bytes -- and
one that is not refused, counted and said. So a node may rely on what it is
given being its type, and needs no check of its own for that. A device's
own bytes are not a type's encoding; ``format`` says how to read them, and
the node at the edge turns them into the type it sends.

Only towards the ground is anything added: a string or bytes, which has no width
of its own, is sent the ground after a count of its bytes, chosen for each
source by ``Telemetered::count`` in ``tcslib-protocol/src/graph/telemetry.rs``
and written by ``counted`` in ``tcslib-protocol/src/link_type.rs``. A node never
writes the count itself. Each configuration's own XTCE, which ``tcs-verify
--xtce`` writes, says what each node sends the ground, and it takes that from
``carries``, so a new type that answers ``carries`` needs nothing more there.

Adding a Payload Type
=====================
In ``tcslib-protocol``
----------------------
The module, ``tcslib-protocol/src/node/payload/<type>/mod.rs``, registered with
a ``pub mod`` in ``node/payload/mod.rs``, holds:

- ``pub const TYPE``, the type's spelling;
- a ``Params`` struct of its attributes;
- ``group_kind_of``, which checks the attributes it was given and turns them
  into the endpoint the vehicle opens. Its errors name the attribute and say
  what to write instead;
- ``judged``, which looks at what is at the node's endpoint and says whether
  it is a simulated device (tcs-sim's), a dummy device (the kernel's) or a
  real one. The judgement is made from what is there, never from a file,
  and ``--simulate`` decides which are allowed;
- its tests.

The type is then made known to the rest of tcslib-protocol:

``src/types.rs``
    A ``DHType`` variant, added to ``DHType::ALL`` with the array's length
    changed, and to ``spelling``, ``article`` and ``spellings``; an
    ``EndpointConfig`` variant, in ``kind`` and ``as_kind``; and an arm in
    ``kind_of_terms``. ``spellings`` is a hand-written string, so read it
    after the change.

``src/endpoint_config.rs``
    Only for an interface tcspecial has not opened before: a ``GroupKind``,
    an arm in ``endpoint_of``, and any new terms in ``LinkTerms``.

``src/graph/terminals.rs``
    An arm in ``terminals_of``.

``src/graph/link_types.rs``
    A branch in ``carries`` if the type's terminals are fixed or take any
    type. Nothing for an ordinary device.

``src/graph/reach.rs``
    ``kind_of``, ``locators``, ``put_term`` for new attributes, and
    ``cannot_be_asked`` and ``cannot_be_written`` where they apply. If the
    type may give ``sim_address``, ``simulated_over_udp`` and the refusal
    text that lists the types that may.

``src/graph/built.rs``
    ``OPTIONAL_TYPES``, with its length, in the order tcspecial's ``BUILT``
    follows.

``src/graph/interface.rs``
    ``sends``, if the type sends nothing until it is read, and its test.

``src/graph/check.rs``
    ``claims_of``, which says what the node holds that no other node may,
    and the minimum attributes the tests' ``node`` gives it.

``src/simulate.rs``
    ``judged_at``, and ``tcs_sim_makes`` if the kernel makes a dummy device
    for it.

``src/sim_config.rs`` and ``src/parameters.rs``
    ``simulable``, and ``endpoint_parameters``, the text of its panel.

Several of these are lists written by hand that repeat ``DHType``:
``spellings``, ``reach::kind_of``, ``interface::sends``, ``tcs_sim_makes``
and the ``sim_address`` texts. A ``match`` on ``DHType`` or
``EndpointConfig`` that misses the new variant is a compile error, but these
are not, so go through them by name.

In ``tcspecial``
----------------
The module, ``tcspecial/src/node/payload/<type>/mod.rs``, holds an endpoint
for the device, conventionally ``XEndpoint`` with ``new`` and
``try_clone``, implementing the traits in ``tcspecial/src/endpoint.rs``:
``EndpointWaitable`` (the file descriptor to wait on, and what to do when it
is ready), ``EndpointReadable`` and ``EndpointWritable``. ``node/payload/can``
is a complete one to read.

The endpoint is called from the main loop for as long as tcspecial runs, so
it allocates nothing there: every buffer it reads or writes into is made
when it opens, or passed in. Where an allocation in the loop cannot be
avoided, a comment at that line says so and why.

Each payload type is a Cargo feature, so that a vehicle is built with only
the types it flies. In ``tcspecial/Cargo.toml``, add the type to the
``default`` features and give it a line of its own, naming any type it is
built on: ``i2c_tmass = ["i2c"]``. In ``node/payload/mod.rs``, add the
module under ``#[cfg(feature = "<type>")]``, add the type to ``BUILT`` in
``OPTIONAL_TYPES``'s order, add a ``compile_error!`` for any platform it
cannot be built for, and add it to the feature list of
``every_type_is_built_by_default``. In ``endpoint.rs``, add a gated ``use``
and an arm in both ``bind_endpoint_pair`` and ``hearing_endpoint_pair``.

``tcspecial --types`` prints ``BUILT``, and ``make check`` builds tcspecial
once for each type with that type's feature alone. So every ``use`` and
every arm for the type must be gated by its feature, or that build fails.

In ``tcs-sim``
--------------
The module, ``tcs-sim/src/node/payload/<type>/mod.rs``, holds a function
``run_<type>_node``, which plays the device: it answers as the device would,
from the simulator file's settings and, for a sensor joined to the physics
model, from the world. There is no trait. ``tcs-sim/src/payload.rs`` runs it
from ``run_kind``, by a ``PayloadProtocol`` variant, and
``tcs-sim/src/endpoint.rs`` maps the node's endpoint and ``sim_address`` to
that protocol in ``payload_config_attached``, and describes it in
``endpoint_description``.

A simulated device must behave as the real one does where tcspecial can
tell: the same bytes, the same timing within reason, the same errors. One
that is easier to talk to than the real part hides the bugs it is there to
find.

Adding a Vehicle Type
=====================
A vehicle type has no device, so it has no Cargo feature, no endpoint, no
judgement and no simulated device. Instead:

- in tcslib-protocol, its module in ``node/vehicle/`` with ``params_of`` and its
  checks, registered in ``node/vehicle/mod.rs``; its name in
  ``VEHICLE_TYPES`` in ``src/graph/mod.rs``; its attribute rules in
  ``graph/check.rs``; its terminals in ``terminals_of``; and its branch in
  ``carries``, which it must have;
- in tcspecial, its module in ``node/vehicle/``, implementing ``Port``
  (``src/runtime/fabric.rs``), and arms in ``open``, ``described`` and
  ``is_polled`` in ``src/runtime/system.rs``, which match the type by name.

``types_in`` in ``tcslib-protocol/src/graph/mod.rs`` is the one statement of
which types each section of a configuration takes, built from ``DHType::ALL``,
the app kinds, the gateway kinds and ``VEHICLE_TYPES``, and the schema tests
hold the schemas to it.

Adding a Compute Function
=========================
A function is the ``compute`` type's ``function``. In tcspecial it is a file,
``src/node/vehicle/compute/<function>.rs``, implementing ``Function``:
``give``, which takes a datum at one of its inputs and may produce one;
``set``, for an attribute the ground may change, which refuses by default;
``held``, what it last produced, ``None`` by default; and ``name``. It is
registered with a ``pub mod`` and an arm in ``make``. Like an endpoint, it
runs in the main loop and allocates nothing there.

In tcslib-protocol, ``node/vehicle/compute/mod.rs`` holds the function's
``Computation`` variant and ``function``, ``functions`` (a hand-written
list), ``attributes_of``, ``params_of`` with its ``Params``, and
``produces``. Its terminals' types come from ``carries``, which gives a
function ``type-in`` and ``type-out``, both required, unless it has a branch
of its own as ``vote`` and ``to_kelvin`` have. A function that narrows
which types it takes, as ``process`` takes only types with a text form
(``tcslib-protocol/src/link_text.rs``), refuses the others in ``params_of``,
naming the attribute and the types it may be.

tcs-gui offers a function as a node of its own, from
``tcs-gui/share/nodes/vehicle/compute/<function>.yaml`` with
``sets: {function: <function>}``. The Python ground needs nothing for it:
how it shows what a node sends follows from the type, in ``form.py``.

Adding a Gateway Type
=====================
A gateway type is a ``GatewayKind`` in ``tcslib-protocol/src/gateway.rs``, in
``ALL``, ``name``, ``is_built``, ``on_bus``, ``framed`` and ``node_type``. If
its ground decodes what it is sent by XTCE, as Yamcs does, the count before a
string or bytes must be XTCE's, so ``Telemetered::count`` must give it
``Count::Xtce``. The Python ground lists gateway types by hand, in
``GATEWAY_BASES`` and ``ON_BUS`` in ``python/tcslib_protocol/configuration.py``.
Its description in tcs-gui is at the top of ``tcs-gui/share/nodes/``, and since it
has no node module, it is one of the exemptions in
``every_description_has_its_rust`` in ``tcs-gui/src/nodes.rs``.

The Editor and the Schemas
==========================
A node type is offered in tcs-gui's palette from a description,
``tcs-gui/share/nodes/<family>/<type>.yaml``: its ``node`` and ``type``, an
``about`` sentence, which terminals it has, and its ``attributes``, each with
``name``, ``required``, an optional ``kind`` and its own ``about``. The
terminal flags are still spelled by their old names, ``payload-sink``,
``payload-source``, ``gateway-sink`` and ``gateway-source``, which
``tcs-gui/src/nodes.rs`` reads as ``world-in``, ``world-out``, ``moc-in``
and ``moc-out``. Attributes that state a terminal's type, such as
``world-out-type``, are listed as any other attribute is. The description
is checked against ``share/schema/node.schema.json`` and ``node.xsd``, and
its path is added to the sorted list in ``the_shipped_descriptions_read``.

A configuration is written in YAML or XML, and the two spellings must say
the same thing. So a new type goes into both configuration schemas,
``tcslib-protocol/schema/tcspecial.schema.json`` and ``tcspecial.xsd``: its name
in the section's type list, and each new attribute in the JSON node
definitions and the XSD's ``nodeWords``. A payload type also goes into both
simulator schemas, ``tcspecial-sim.schema.json`` and ``tcspecial-sim.xsd``.
The Python ground reads its type lists from the JSON schema, so it follows.

Drawing It
==========
How a type is drawn is in ``tcs-diagram/src/style.rs``: a ``NodeType``
variant, in ``ALL`` with its length, ``spelling``, ``family``,
``is_payload``, ``attributes`` (which must be the description's, in the
same order and with the same ``required``), and ``appearance``, its edge,
weight, size and face. Its colour is the pane's next one, given by
``came_later`` in ``look.rs``, which puts the new type after every existing
one so that no colour already in use moves.

Colour is never the only thing that tells two node types apart, for readers
who do not see it. A test in ``style.rs`` holds every pair of types to two
differences that are not colour, and ``no_two_types_look_the_same`` holds
them to looking different at all.
The payload edges and weights are nearly all taken, so a new payload type
may need a new ``Edge``, as the ``_tmass`` types needed ``DashDotted``.

Documenting It ============== A type is not finished until it is documented, and
a requirement in ``tcspecial-design.rst`` that the code does not meet is a bug
in one or the other:

- ``docs/tcspecial-design.rst``: a row in the table under "Every terminal
  carries one type", the type's requirements where its kind's are, its
  feature in "Which Node Types Are Built", and, for a payload, the judgement
  table and the ``sim_address`` requirement;
- ``docs/manual.rst``: a section for the type, with a ``Keywords`` line,
  its attributes and an example; the simulator's list of types; and the
  list of features under "Building tcspecial for a vehicle". Every
  configuration example in the manual is verified by a test, so an example
  that does not load fails it;
- ``CLAUDE.md``, if the type may give ``sim_address``;
- ``examples/README.md``, and both spellings of an example, if one uses it.

Each RST and Markdown page is turned into HTML by ``make`` in its directory,
and the HTML is committed with it; ``make html-check`` fails while a page
is out of date.

Testing It
==========
Each module has its tests beside it, in all three crates. A test for a fix
should fail without the fix: check that it does.

Many tests enumerate every type, and a new type that was missed somewhere
fails one of them: the schema tests in ``tcs-gui/tests/schema.rs``, which
hold the schemas to ``types_in`` and ``DHType::ALL``; the description tests
in ``tcs-gui/src/nodes.rs``; the drawing tests in ``tcs-diagram``;
``the_features_are_the_optional_types`` and ``tcspecial/tests/types.rs``;
and ``tcs-verify/tests/examples.rs``. No test holds ``NodeType::ALL`` to
``types_in``, so a type missing from the drawing is caught only through the
description tests.

Before a new type is committed::

   cargo fmt --all
   cargo test --workspace --exclude ping_example
   cargo clippy --workspace --all-targets
   make check
   make html-check

``make check`` builds each node type alone, compares the two spellings of
every shipped configuration, and checks the HTML.

A Checklist
===========
For a payload type ``foo``:

1. tcslib-protocol: ``node/payload/foo/mod.rs``, with ``TYPE``, its ``Params``,
   ``group_kind_of``, ``judged`` and tests, and ``pub mod foo``.
2. ``types.rs``: ``DHType`` and ``ALL``, ``spelling``, ``article``,
   ``spellings``, ``EndpointConfig``, ``kind``, ``as_kind``,
   ``kind_of_terms``; and ``endpoint_config.rs`` for a new interface.
3. ``graph/terminals.rs``, and ``graph/link_types.rs`` if its types are
   fixed or any.
4. ``graph/reach.rs``, ``graph/built.rs``, ``graph/interface.rs``,
   ``graph/check.rs``, ``simulate.rs``, ``sim_config.rs`` and
   ``parameters.rs``.
5. tcspecial: the endpoint, the feature in ``Cargo.toml``, ``BUILT`` and
   the rest of ``node/payload/mod.rs``, and both dispatchers in
   ``endpoint.rs``, every one gated by the feature.
6. tcs-sim: ``run_foo_node``, ``PayloadProtocol`` and ``run_kind``, and
   ``endpoint.rs``.
7. tcs-gui: the description, its place in the list of descriptions, both
   configuration schemas and both simulator schemas.
8. tcs-diagram: ``NodeType`` and its ``appearance``, two ways unlike every
   other type that are not colour.
9. The design, the manual, ``CLAUDE.md`` and the examples, and their HTML.
10. The checks above.

For a vehicle type, leave out the device, the feature, the endpoint, the
judgement and the simulator, and add its ``VEHICLE_TYPES`` entry, its
``carries`` branch, its ``Port`` and its arms in ``runtime/system.rs``.
