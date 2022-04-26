#!/usr/bin/env bash

pause_node1() {
  echo "Pausing node1"
  docker stop e2e-tendermint-1-1
}

#stop docker container tendermint-2-1 and restart it after 10s
pause_node2() {
  echo "Pausing node2"
  docker stop e2e-tendermint-2-1
  sleep 10
  docker start e2e-tendermint-2-1
}

# restart docker container tendermint-3-1 and restart it after 10s
pause_node3() {
  echo "Pausing node3"
  docker stop e2e-tendermint-3-1
  sleep 10
  docker start e2e-tendermint-3-1
}
