nix run github:cargo2nix/cargo2nix --max-jobs $CPUCORES -- -f docker-nix/Cargo.nix.new
mv docker-nix/Cargo.nix.new docker-nix/Cargo.nix
chown $UINFO docker-nix/Cargo.nix
