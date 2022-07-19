NB_NODES ?= 4
ID_WITH_BALANCES ?=

.PHONY: clean
clean:
	if [ -f "${PWD}/genfiles/docker-compose.json" ]; then \
		docker-compose -f genfiles/docker-compose.json -p e2e down || true; \
	fi
	rm -rf genfiles/

.PHONY: clean-tm-data
clean-tm-data:
	rm -rf genfiles/node*/tendermint/data/
	rm -rf genfiles/node*/persistent-ledger/

.PHONY: build
build: genfiles/build-repo genfiles/build-ledger

genfiles/build-repo: ../../src/**/*
	docker build --ssh default --tag many_local/repo_build ../.. --target builder -f ../build.dockerfile
	@mkdir -p $(dir $@) && touch $@

genfiles/build-ledger: genfiles/build-repo
	docker build --build-arg USER_ID=$$(id -u) --ssh default --tag many/ledger ../.. --target runtime -f ../ledger/Dockerfile
	@mkdir -p $(dir $@) && touch $@

genfiles/build-abci: genfiles/build-repo
	docker build --build-arg USER_ID=$$(id -u) --ssh default --tag many/abci ../.. --target runtime -f ../abci/Dockerfile
	@mkdir -p $(dir $@) && touch $@

genfiles/tendermint-docker:
	docker pull tendermint/tendermint:v0.35.1
	@mkdir -p $(dir $@) && touch $@

genfiles/openssl-docker:
	docker pull alpine/openssl
	@mkdir -p $(dir $@) && touch $@

genfiles/jsonnet-docker:
	docker pull bitnami/jsonnet
	@mkdir -p $(dir $@) && touch $@

genfiles/docker-compose.json: genfiles/jsonnet-docker
	docker run --user $$(id -u) -it --rm -v "${PWD}:/volume:ro" -v "${PWD}/genfiles:/genfiles" bitnami/jsonnet \
		/volume/docker-compose.jsonnet \
		--tla-code nb_nodes=$(NB_NODES) \
		--tla-code user=$$(id -u) \
		--tla-code id_with_balances=\"$(ID_WITH_BALANCES)\" \
		-o /$@

genfiles/node%: genfiles/openssl-docker
	mkdir -p "$@/tendermint"
	docker run --user $$(id -u) -it --rm -v "${PWD}/$@/tendermint:/tendermint" tendermint/tendermint:v0.35.1 init validator
	docker run --user $$(id -u) -it --rm -v ${PWD}/$@/:/export alpine/openssl genpkey -algorithm Ed25519 -out /export/ledger.pem
	docker run --user $$(id -u) -it --rm -v ${PWD}/$@/:/export alpine/openssl genpkey -algorithm Ed25519 -out /export/abci.pem
	mkdir -p "$@/persistent-ledger"
	cp ledger_state.json $@/ledger_state.json5


genfiles/generate-tendermint-e2e-config:
	I=0; while [[ $$I -lt ${NB_NODES} ]]; do \
		echo $$I genfiles/node$$I; \
		make genfiles/node$$I; \
		(( I = I + 1 )); \
	done
	sh update_config.sh -c "genfiles/node%/tendermint/config" -i tendermint-% $(NB_NODES)
	mkdir -p $(dir $@) && touch $@

.PHONY: start-all-nodes stop-node start-ledger-local
start-ledger-local: build-ledger
	docker run -d --name ledger -p 8000:8000 --volume ${PWD}/../volumes/ledger:/persistent many/ledger

start-nodes: genfiles/build-abci genfiles/build-ledger genfiles/generate-tendermint-e2e-config genfiles/docker-compose.json
	docker-compose -f genfiles/docker-compose.json -p e2e up

stop-nodes:
	docker-compose -f genfiles/docker-compose.json -p e2e down

start-docker-compose: start-nodes

start-nodes-background: genfiles/build-abci genfiles/build-ledger genfiles/generate-tendermint-e2e-config genfiles/docker-compose.json
	docker-compose -f genfiles/docker-compose.json -p e2e up --detach

start-node-background: genfiles/build-abci genfiles/build-ledger genfiles/generate-tendermint-e2e-config genfiles/docker-compose.json
	docker-compose -f genfiles/docker-compose.json -p e2e up --detach tendermint-$(NODE)
	docker-compose -f genfiles/docker-compose.json -p e2e up --detach abci-$(NODE)
	docker-compose -f genfiles/docker-compose.json -p e2e up --detach ledger-$(NODE)

chaos:
	sh chaos.sh
