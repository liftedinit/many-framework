
.PHONY: clean
clean:
	if [ -f "${PWD}/genfiles/docker-compose.json" ]; then \
		docker-compose -p e2e down; \
	fi
	rm -rf genfiles/

.PHONY: clean-tm-data
clean-tm-data:
	rm -rf genfiles/node*/tendermint/data/
	rm -rf genfiles/node*/persistent-ledger/

.PHONY: build
build: build-repo build-ledger

build-repo:
	docker build --ssh default --tag many_local/repo_build ../.. --target builder -f ../build.dockerfile

build-ledger: build-repo
	docker build --build-arg USER_ID=$$(id -u) --ssh default --tag many/ledger ../.. --target runtime -f ../ledger/Dockerfile

build-abci: build-repo
	docker build --build-arg USER_ID=$$(id -u) --ssh default --tag many/abci ../.. --target runtime -f ../abci/Dockerfile

.PHONY: start-ledger-local
start-ledger-local: build-ledger
	docker run -d --name ledger -p 8000:8000 --volume ${PWD}/../volumes/ledger:/persistent many/ledger

.PHONY: tendermint-docker openssl-docker jsonnet-docker

tendermint-docker:
	docker pull tendermint/tendermint:v0.35.1

openssl-docker:
	docker pull alpine/openssl

jsonnet-docker:
	docker pull bitnami/jsonnet

NB_NODES ?= 4

genfiles/docker-compose.json: jsonnet-docker
	docker run -it --rm -v "${PWD}:/volume:ro" bitnami/jsonnet \
		/volume/docker-compose.jsonnet --tla-code nb_nodes=$(NB_NODES) --tla-code user=$$(id -u) > $@

genfiles/node%: tendermint-docker openssl-docker
	mkdir -p "$@/tendermint"
	docker run --user $$(id -u) -it --rm -v "${PWD}/$@/tendermint:/tendermint" tendermint/tendermint:v0.35.1 init validator
	docker run --user $$(id -u) -it --rm -v ${PWD}/$@/:/export alpine/openssl genpkey -algorithm Ed25519 -out /export/ledger.pem
	docker run --user $$(id -u) -it --rm -v ${PWD}/$@/:/export alpine/openssl genpkey -algorithm Ed25519 -out /export/abci.pem
	mkdir -p "$@/persistent-ledger"
	cp ledger_state.json $@/ledger_state.json


generate-e2e-config:
	I=0; while [[ $$I -lt ${NB_NODES} ]]; do \
		echo $$I genfiles/node$$I; \
		make genfiles/node$$I; \
		(( I = I + 1 )); \
	done
	sh update_config.sh -c "genfiles/node%/tendermint/config" -i tendermint-% $(NB_NODES)

.PHONY: start-all-nodes stop-node
start-all-nodes:
	docker-compose -f genfiles/docker-compose.json -p e2e up

stop-node:
	docker-compose -f genfiles/docker-compose.json -p e2e down 

start-docker-compose: build-abci build-ledger generate-e2e-config genfiles/docker-compose.json start-all-nodes

chaos: 
	sh chaos.sh 
