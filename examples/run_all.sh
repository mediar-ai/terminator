#!/usr/bin/env bash
# Run all Python scripts in this directory except gnome* and macos* ones.
set -u

cd "$(dirname "$0")"

# Prefer the project venv's Python if available.
if [ -x ".venv/Scripts/python.exe" ]; then
    PYTHON=".venv/Scripts/python.exe"
elif [ -x ".venv/bin/python" ]; then
    PYTHON=".venv/bin/python"
else
    PYTHON="python"
fi

run_script() {
    echo "=== Running $1 ==="
    "$PYTHON" "$1"
    echo "=== Finished $1 (exit $?) ==="
    echo
}

# Run everything except vlc_auto_player.py first...
for script in *.py; do
    # Skip other-OS demos, _*-prefixed scratch/diagnostic scripts (e.g.
    # _diag_vlc.py, which also opens VLC), and vlc_auto_player.py (run last).
    case "$script" in
        gnome*|macos*|_*|vlc_auto_player.py) continue ;;
    esac
    run_script "$script"
done

# ...then run vlc_auto_player.py last (it opens VLC and streams for a while).
if [ -f vlc_auto_player.py ]; then
    run_script vlc_auto_player.py
fi
