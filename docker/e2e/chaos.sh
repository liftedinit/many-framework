#!/bin/bash 
RED='\033[0;31m'
NC='\033[0m'
CYAN='\033[0;36m'
PURPLE='\033[0;35m'
GREEN='\033[0;32m'

c=0
no_sleep () {
    echo "${GREEN}RUN TENDERMINT NO SLEEP CHAOS TEST ${GREEN}"
    while [ $c -le 14 ]
    do
      echo "${RED}STOP TENDERMINT NODE 0 $c times${RED}"
      ((c++))
      docker stop e2e-tendermint-0-1 
      echo "${GREEN}START TENDERMINT NODE 0 $c times${GREEN}"
	    docker start e2e-tendermint-0-1

      echo "${RED}STOP TENDERMINT NODE 1  $c times${RED}"
      docker stop e2e-tendermint-1-1 
      echo "${GREEN}START TENDERMINT NODE 1  $c times${GREEN}"
      docker start e2e-tendermint-1-1 

      echo "${RED}STOP TENDERMINT NODE 2  $c times${RED}"
      docker stop e2e-tendermint-2-1 
      echo "${GREEN}START TENDERMINT NODE 2  $c times${GREEN}"
      docker start e2e-tendermint-2-1 

      echo "${RED}STOP TENDERMINT NODE 3  $c times${RED}"
      docker stop e2e-tendermint-3-1 
      echo "${GREEN}START TENDERMINT NODE 3  $c times${GREEN}"
      docker start e2e-tendermint-3-1 

      echo "${NC}NO SLEEP TEST OVER${NC}"
    done
}

rapid_fire () {
    echo "${RED}RUN TENDERMINT RAPID FIRE CHAOS ${RED}"
    while [ $c -le 30 ]
    do
      echo "${RED}STOP TENDERMINT CHAOS TEST $c times${RED}"
      ((c++))
      echo "${RED}STOP TENDERMINT NODE 1${RED}"
      docker stop e2e-tendermint-1-1 
	    sleep 1
      echo "${CYAN}START TENDERMINT NODE 1  $c times${CYAN}"
	    docker start e2e-tendermint-1-1
      sleep 1

      echo "${CYAN}STOP TENDERMINT NODE 0  $c times${CYAN}"
      docker stop e2e-tendermint-0-1 
	    sleep 1
      echo "${CYAN}START TENDERMINT NODE 0  $c times${CYAN}"
      docker start e2e-tendermint-0-1 
	    sleep 1

      echo "${CYAN}STOP TENDERMINT NODE 3  $c times${CYAN}"
      docker stop e2e-tendermint-3-1 
	    sleep 1
      echo "${CYAN}START TENDERMINT NODE 3  $c times${CYAN}"
      docker start e2e-tendermint-3-1 
	    sleep 1

      echo "${CYAN}STOP TENDERMINT NODE 2  $c times${CYAN}"
      docker stop e2e-tendermint-2-1 
	    sleep 1
      echo "${CYAN}Start TENDERMINT NODE 2  $c times${CYAN}"
      docker start e2e-tendermint-2-1 
	    sleep 1

    done
    echo "${NC}RAPID FIRE TEST OVER${NC}"
}

random_time () {
    echo "${PURPLE}RUN TENDERMINT CHAOS TEST WITH RANDOM INTERVALS ${PURPLE}"
    while [ $c -le 45 ]
    do
      echo "${RED}STOP TENDERMINT CHAOS TEST $c times${RED}"
      ((c++))
  
      echo "${RED}STOP TENDERMINT NODE 1  $c times${RED}"
      docker stop e2e-tendermint-1-1 
	    sleep 2
      echo "${PURPLE}START TENDERMINT NODE 1  $c times${PURPLE}"
	    docker start e2e-tendermint-1-1
      sleep 4
  
      echo "${RED}STOP TENDERMINT NODE 0  $c times${RED}"
      docker stop e2e-tendermint-0-1 
	    sleep 3
      echo "${PURPLE}START TENDERMINT NODE 0  $c times${PURPLE}"
      docker start e2e-tendermint-0-1 
	    sleep 3
  
      echo "${RED}STOP TENDERMINT NODE 0  $c times${RED}"
      docker stop e2e-tendermint-3-1 
	    sleep 1
      echo "${PURPLE}START TENDERMINT NODE 3  $c times${PURPLE}"
      docker start e2e-tendermint-3-1 
	    sleep 1
  
      echo "${RED}STOP TENDERMINT NODE 2  $c times${RED}"
      docker stop e2e-tendermint-2-1 
	    sleep 3
      echo "${PURPLE}STOP TENDERMINT NODE 0  $c times${PURPLE}"
      docker start e2e-tendermint-2-1 
	    sleep 3
      echo "${NC}RANDOM TIME TEST OVER${NC}"
    done
}

no_sleep
rapid_fire
random_time
