#!/bin/sh
# Creates the sandbox the test bed runs in (/tmp/ox). Safe: nothing outside /tmp/ox is touched, and the test
# compositor, shell and config never see your real session.
set -e
mkdir -p /tmp/ox/cfg/oxide /tmp/ox/data/oxide /tmp/ox/cache
cp -f "$(dirname "$0")"/lib.sh "$(dirname "$0")"/up.sh "$(dirname "$0")"/t.py "$(dirname "$0")"/frames.py /tmp/ox/
# the installed shell and wallpapers (run contrib/install.sh first, or point these at contrib/)
for d in shells wallpapers; do
  src="$HOME/.local/share/oxide/$d"; [ -d "$src" ] || src="$(dirname "$0")/../../contrib/$d"
  ln -sfn "$(realpath "$src")" /tmp/ox/data/oxide/$d
done
echo "test bed ready in /tmp/ox"
