nix build $3
cp $(readlink result) $2
chown $1 $2
rm result
