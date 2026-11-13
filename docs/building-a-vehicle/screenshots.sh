#!/bin/sh
# Draws each stage of "Building a Vehicle" as tcsgui draws it, into
# docs/shots/vehicle-stage-NN.png, for the guide's figures.
#
#   docs/building-a-vehicle/screenshots.sh <tcsgui> <display>
#
# <tcsgui> is the program, built; <display> an X display that is not the
# person's own, such as a Xephyr started with
#
#   Xephyr :5 -screen 1600x1000
#
# so that no window opens on their screen. Each stage is opened in a tcsgui
# of its own, with a home and state directory of its own so that nothing
# the person has set changes the drawing, given time to draw, captured with
# ffmpeg, and closed. Run from the repository's top directory.
set -eu

if [ $# -ne 2 ]; then
	echo "usage: screenshots.sh <tcsgui> <display>" >&2
	exit 2
fi
tcsgui=$1
display=$2
out=docs/shots
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# Tells whether the display is there, and stops, saying how to start one,
# if not.
if ! DISPLAY=$display xdpyinfo > /dev/null 2>&1; then
	echo "screenshots.sh: no X display at $display: start one, such" \
	     "as Xephyr $display -screen 1600x1000, and run this again" >&2
	exit 1
fi

# Only the numbered stages: stage 10's cFS and Yamcs versions draw as stage
# 10 does, and the guide shows no drawing of them.
for stage in docs/building-a-vehicle/stage-[0-9][0-9]; do
	n=${stage##*stage-}
	# Without WAYLAND_DISPLAY, so that tcsgui opens on the X display
	# given rather than on the person's Wayland one.
	env -u WAYLAND_DISPLAY HOME="$work/home" XDG_STATE_HOME="$work/state" \
		DISPLAY="$display" "$tcsgui" "$stage/deck.ytcs" \
		> "$work/tcsgui.log" 2>&1 &
	pid=$!
	# Long enough for tcsgui to lay the diagram out and draw it; a
	# capture taken before that is black.
	sleep 15
	ffmpeg -loglevel error -y -f x11grab -draw_mouse 0 \
		-video_size 1373x820 -i "$display+0,0" -frames:v 1 \
		"$out/vehicle-stage-$n.png"
	kill "$pid"
	wait "$pid" 2> /dev/null || true
	echo "$out/vehicle-stage-$n.png"
done
