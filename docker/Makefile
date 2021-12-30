
build: build-repo build-ledger

build-repo:
	docker build --ssh default --tag local/repo_build .. --target builder -f build.dockerfile

build-ledger: build-repo
	docker build --ssh default --tag omni/ledger .. --target runtime -f ledger/Dockerfile

start-ledger-local: build-ledger
	docker run -d --rm --name ledger -p 8000:8000 --volume ${PWD}/volumes/ledger:/persistent omni/ledger
