
clean:
	rm -rf genfiles/
	docker-compose down

build: build-repo build-ledger

build-repo:
	docker build --ssh default --tag omni_local/repo_build ../.. --target builder -f ../build.dockerfile

build-ledger: build-repo
	docker build --ssh default --tag omni/ledger ../.. --target runtime -f ../ledger/Dockerfile

build-abci: build-repo
	docker build --ssh default --tag omni/abci ../.. --target runtime -f ../abci/Dockerfile

start-ledger-local: build-ledger
	docker run -d --name ledger -p 8000:8000 --volume ${PWD}/../volumes/ledger:/persistent omni/ledger

tendermint-docker:
	docker pull tendermint/tendermint:v0.35.0

openssl-docker:
	docker pull alpine/openssl

genfiles/node%: tendermint-docker openssl-docker
	mkdir -p "$@/tendermint"
	docker run -it --rm -v "${PWD}/$@/tendermint:/tendermint" tendermint/tendermint:v0.35.0 init validator
	docker run -it --rm -v ${PWD}/$@/:/export alpine/openssl genpkey -algorithm Ed25519 -out /export/ledger.pem
	docker run -it --rm -v ${PWD}/$@/:/export alpine/openssl genpkey -algorithm Ed25519 -out /export/abci.pem
	cp ledger_state.json $@/ledger_state.json

generate-tendermint-e2e-config: genfiles/node0 genfiles/node1 genfiles/node2 genfiles/node3
	sh update_config.sh -c "genfiles/node%/tendermint/config" -i tendermint-% 0 3

start-docker-compose: build-abci build-ledger generate-tendermint-e2e-config
	docker-compose up
