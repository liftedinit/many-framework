
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

single-node:
	bash scripts/run.sh

.PHONY: check-clippy
check-clippy:
	cargo fmt --all -- --check
	cargo clippy --all-targets --all-features -- -D clippy::all

build-all-test:
	cargo build --lib --tests --all-features --all-targets

run-all-unit-test:
	cargo test --lib --all-targets --all-features

run-all-doc-test:
	cargo test --all-features --doc

