echo "filter-syscalls = false" >> /etc/nix/nix.conf
nix build --max-jobs $CPUCORES $2
cp $(readlink result) $1
chown $UINFO $1
rm result
