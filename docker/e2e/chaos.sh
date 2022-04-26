#!/bin/bash 
RED='\033[0;31m'
NC='\033[0m'
CYAN='\033[0;36m'
c=1
fun1 () {
    echo -e "${RED}RUN TENDERMINT CHAOS TEST 1${RED}"
    while [ $c -le 15 ]
    do
    echo "${RED}STOP TENDERMINT CHAOS TEST $c times${RED}"
    ((c++))
    docker stop e2e-tendermint-1-1 
	  sleep 2
        echo "${CYAN}START TENDERMINT NODE 1  $c times${CYAN}"

	  docker start e2e-tendermint-1-1
    sleep 4

    docker stop e2e-tendermint-0-1 
	  sleep 3
        echo "${CYAN}START TENDERMINT NODE 0  $c times${CYAN}"
    docker start e2e-tendermint-0-1 
	  sleep 3

        docker stop e2e-tendermint-3-1 
	  sleep 1
        echo "${CYAN}START TENDERMINT NODE 3  $c times${CYAN}"
    docker start e2e-tendermint-3-1 
	  sleep 1

        echo "${CYAN}START & STOP TENDERMINT NODE 2  $c times${CYAN}"
       docker stop e2e-tendermint-2-1 
	  sleep 3
    docker start e2e-tendermint-2-1 
	  sleep 3

  done
    [[ "@" ]] && echo "options: $@"
}

fun1