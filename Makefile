
.PHONY: clean
clean:
	rm -rf coverage/
	rm -rf target/

target/bin/grcov:
	cargo install grcov --root target/

target/debug/:
	cargo build --all-features

coverage/report.lcov: target/bin/grcov target/debug/
	make generate-test-coverage
	target/bin/grcov src --binary-path target/debug/ -s . --keep-only 'src/**' --prefix-dir $PWD -t lcov --branch --ignore-not-existing -o coverage/report.lcov

generate-lcov-coverage: coverage/report.lcov

generate-test-coverage:
	RUSTFLAGS="-C instrument-coverage" LLVM_PROFILE_FILE="coverage/lcov-%p-%m.profraw" make run-all-unit-test

coverage/index.html: target/bin/grcov generate-test-coverage coverage/report.lcov
	target/bin/grcov src --binary-path target/debug/ -s . --keep-only 'src/**'  -t html --branch --ignore-not-existing -o ./coverage/

.PHONY: code-coverage
code-coverage: coverage/index.html

.PHONY: single-node
single-node:
	bash scripts/run.sh

.PHONY: check-clippy check-fmt check-lint
check-clippy:
	cargo fmt --all -- --check
check-fmt:
	cargo clippy --all-targets --all-features -- -D clippy::all
check-lint: check-clippy check-fmt

.PHONY: build-all-test
build-all-test:
	cargo build --lib --tests --all-features --all-targets

.PHONY: run-all-unit-test run-all-doc-test
run-all-unit-test:
	cargo test --lib --all-targets --all-features

run-all-doc-test:
	cargo test --all-features --doc

.PHONY: ci
ci: check-lint build-all-test run-all-unit-test run-all-doc-test

.PHONY: hybrid-images
hybrid-images:
	cd docker/e2e && \
	sed -i 's/0\.24\.0\-pre\.2/0\.25\.0/g' ../../src/many-abci/Cargo.toml && \
	make Cargo.nix many/many-abci many/many-ledger && \
	docker tag lifted/many-abci:latest lifted/many-abci:v0.34.21 && \
	sed -i 's/0\.25\.0/0\.24\.0\-pre\.2/g' ../../src/many-abci/Cargo.toml && \
	make Cargo.nix many/many-abci && \
	docker tag lifted/many-abci:latest lifted/many-abci:v0.35.4
