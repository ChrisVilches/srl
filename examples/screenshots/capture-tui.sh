#!/bin/sh
# Captures screens of srl-tui as text with colors, one .ansi file per shot,
# for shoot.mjs to draw. Needs tmux and uv; tried on Linux only.
#
#   ./capture-tui.sh STORE OUT
#
# STORE is the demo store to study (a copy of store/, see README.md), OUT
# the folder the .ansi files go to. No card is rated: a shot only shows a
# card, and its answer when the shot asks for it.
set -eu

store=$(cd "$1" && pwd)
out=$(mkdir -p "$2" && cd "$2" && pwd)
here=$(cd "$(dirname "$0")" && pwd)
frontend="$here/../python-frontend"

# The terminal size, in columns and rows, of every shot.
cols=108
rows=38

# capture NAME SELECTOR REVEAL: start srl-tui on SELECTOR, press space if
# REVEAL is 1, and save the screen as OUT/NAME.ansi.
capture() {
  tmux -L srl-shots new-session -d -s "$1" -x "$cols" -y "$rows" \
    -e SRL_DIR="$store" -e TERM=xterm-256color -e COLORTERM=truecolor \
    "cd '$frontend' && uv run srl-tui $2; sleep 600"
  sleep 3
  if [ "$3" = 1 ]; then
    tmux -L srl-shots send-keys -t "$1" Space
    sleep 1
  fi
  tmux -L srl-shots capture-pane -t "$1" -e -p > "$out/$1.ansi"
  tmux -L srl-shots kill-session -t "$1"
  echo "$out/$1.ansi"
}

capture tui-1-rust-question in:rust-ownership 0
capture tui-2-rust-answer in:rust-ownership 1
capture tui-3-japanese-grammar in:grammar 1
capture tui-4-japanese-words in:vocabulary 1
capture tui-5-chinese-tones in:tones 1
capture tui-6-electrical in:electrical 1
capture tui-7-geography in:rivers-of-south-america 1
