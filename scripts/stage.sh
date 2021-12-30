#!/usr/bin/env bash
set -upbe

case "$1" in
  1) _name=tendermint-1 ;;
  2) _name=tendermint-2 ;;
  3) _name=tendermint-3 ;;
  4) _name=tendermint-4 ;;
  *) echo "Invalid first argument"; return 1
esac

cd "$(dirname $0)/.."

cargo build

export TMHOME
TMHOME="${PWD}/staging/$_name"

tendermint init validator

tmux kill-session -t "$_name" || true
tmux new-session -s "$_name" -d "tendermint start 2>&1 | tee ~/tendermint.log"
tmux new-window -t "$_name" "./target/debug/omni-ledger --abci --addr 127.0.0.1:8000 --pem ~/Identities/id1.pem --state ./staging/ledger_state.json 2>&1 | tee ~/omni-ledger.log"
tmux new-window -t "$_name" "./target/debug/omni-abci -v --omni 0.0.0.0:8001 --omni-app http://localhost:8000 --omni-pem $HOME/Identities/id1.pem --abci 127.0.0.1:26658 --tendermint http://localhost:26657/ 2>&1 | tee ~/omni-abci.log"
tmux new-window -t "$_name" "$SHELL"

tmux -2 attach-session -t "$_name"

