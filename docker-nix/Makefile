WORKDIR = $(abspath $(dir $(abspath $(dir $$PWD))))
UINFO = "$(shell id -u $(USER)):$(shell id -g $(USER))"
BUILDER_COMMAND = docker run --rm -v $(WORKDIR):/workdir -v nix-store:/nix -w /workdir/docker-nix nixpkgs/nix-flakes bash build-image.sh $(UINFO) $@

.PHONY: clean
clean:
	if [ -d "${PWD}/out/docker-compose.json" ]; then \
		docker-compose -f out/docker-compose.json -p e2e down || true; \
	fi
	rm -rf out

out:
	mkdir -p out

Cargo.nix: ../Cargo.lock
	docker run --rm -v $(WORKDIR):/workdir -v nix-store:/nix -w /workdir nixpkgs/nix-flakes bash generate-cargo-nix.sh $(UINFO)

out/many-ledger.tar.gz: ../src ./Cargo.nix ./flake.nix ./flake.lock out
	$(BUILDER_COMMAND) .#docker-many-ledger

.PHONY: many/many-ledger
many/many-ledger: out/many-ledger.tar.gz
	docker load < out/many-ledger.tar.gz

out/many-abci.tar.gz: ../src ./Cargo.nix ./flake.nix ./flake.lock out
	$(BUILDER_COMMAND) .#docker-many-abci

.PHONY: many/many-abci
many/many-abci: out/many-abci.tar.gz
	docker load < out/many-abci.tar.gz
