# Options
NB_NODES ?= 4
ID_WITH_BALANCES ?=
CPUCORES ?= $$(nproc)
TENDERMINT_VERSION ?= 0.35.4

# Constants
VOLUME = $(abspath $(dir $(abspath $(dir $(abspath $(dir $(abspath $(dir $$PWD))))))))
UINFO = $$(id -u):$$(id -g)
BUILDER_COMMAND = docker run --rm -e CPUCORES=$(CPUCORES) -e UINFO=$(UINFO) -v $(VOLUME):/volume -v nix-store:/nix -w /volume/many-framework/docker/e2e/ nixpkgs/nix-flakes bash build-image.sh $@

.PHONY: clean
clean:
	if [ -d "${PWD}/genfiles/docker-compose.json" ]; then \
		docker-compose -f genfiles/docker-compose.json -p e2e down || true; \
	fi
	rm -rf genfiles

../../Cargo.lock: $(shell find ../../ -name Cargo.toml)
	cd ../..; cargo fetch
	touch $@

Cargo.nix: ../../Cargo.lock
	docker run --rm -e CPUCORES=$(CPUCORES) -e UINFO=$(UINFO) -v nix-store:/nix -v $(VOLUME):/volume -w /volume/many-framework nixpkgs/nix-flakes bash docker/e2e/generate-cargo-nix.sh

genfiles/many-ledger.tar.gz: $(shell find ../../src -type f) Cargo.nix flake.nix flake.lock
	@mkdir -p genfiles
	$(BUILDER_COMMAND) .#docker-many-ledger

.PHONY: many/many-ledger
many/many-ledger: genfiles/many-ledger.tar.gz
	docker load < genfiles/many-ledger.tar.gz

genfiles/many-abci.tar.gz: $(shell find ../../src -type f) Cargo.nix flake.nix flake.lock
	@mkdir -p genfiles
	$(BUILDER_COMMAND) .#docker-many-abci

.PHONY: many/many-abci
many/many-abci: genfiles/many-abci.tar.gz
	docker load < genfiles/many-abci.tar.gz

genfiles/tendermint-docker:
	@mkdir -p genfiles
	docker pull tendermint/tendermint:v$(TENDERMINT_VERSION)
	touch $@

genfiles/openssl-docker:
	@mkdir -p genfiles
	docker pull alpine/openssl
	touch $@

genfiles/jsonnet-docker:
	@mkdir -p genfiles
	docker pull bitnami/jsonnet
	touch $@

genfiles/docker-compose.json: genfiles/jsonnet-docker docker-compose.jsonnet
	docker run --user $$(id -u):$$(id -g) --rm -v "${PWD}:/volume:ro" -v "${PWD}/genfiles:/genfiles" bitnami/jsonnet \
		/volume/docker-compose.jsonnet \
		--tla-code nb_nodes=$(NB_NODES) \
		--tla-code user=$$(id -u) \
		--tla-code id_with_balances=\"$(ID_WITH_BALANCES)\" \
		--tla-code tendermint_tag=\"$(TENDERMINT_VERSION)\" \
		-o /$@

genfiles/node%: genfiles/openssl-docker
	mkdir -p "$@/tendermint"
	docker run --user $$(id -u) -it --rm -v "${PWD}/$@/tendermint:/tendermint" tendermint/tendermint:v$(TENDERMINT_VERSION) init validator
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

.PHONY: start-nodes
start-nodes: many/many-ledger many/many-abci genfiles/generate-tendermint-e2e-config genfiles/docker-compose.json
	docker-compose -f genfiles/docker-compose.json -p e2e up

.PHONE: start-abci-%
start-abci-%:
	docker-compose -f genfiles/docker-compose.json -p e2e up abci-$*

.PHONE: start-ledger-%
start-ledger-%:
	docker-compose -f genfiles/docker-compose.json -p e2e up ledger-$*

.PHONE: start-tendermint-%
start-tendermint-%:
	docker-compose -f genfiles/docker-compose.json -p e2e up tendermint-$*

.PHONY: down-nodes
down-nodes:
	docker-compose -f genfiles/docker-compose.json -p e2e down

.PHONE: down-abci-%
down-abci-%:
	docker-compose -f genfiles/docker-compose.json -p e2e down abci-$*

.PHONE: down-ledger-%
down-ledger-%:
	docker-compose -f genfiles/docker-compose.json -p e2e down ledger-$*

.PHONE: down-tendermint-%
down-tendermint-%:
	docker-compose -f genfiles/docker-compose.json -p e2e down tendermint-$*

.PHONE: stop-nodes
stop-nodes:
	docker-compose -f genfiles/docker-compose.json -p e2e stop

.PHONE: stop-abci-%
stop-abci-%:
	docker-compose -f genfiles/docker-compose.json -p e2e stop abci-$*

.PHONE: stop-ledger-%
start-ledger-%:
	docker-compose -f genfiles/docker-compose.json -p e2e stop ledger-$*

.PHONE: stop-tendermint-%
stop-tendermint-%:
	docker-compose -f genfiles/docker-compose.json -p e2e stop tendermint-$*

.PHONY: start-nodes-dettached
start-nodes-dettached: many/many-ledger many/many-abci genfiles/generate-tendermint-e2e-config genfiles/docker-compose.json
	docker-compose -f genfiles/docker-compose.json -p e2e up --detach
