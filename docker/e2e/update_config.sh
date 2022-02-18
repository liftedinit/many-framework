#!/usr/bin/env bash

get_node_id() {
  local node_id
  node_id="$(jq -r .id < "./genfiles/node$1/tendermint/config/node_key.json")"
  echo "$node_id"
}

update_toml_key() {
  local file section key value temp_file
  file="$1"
  section="$2"
  key="$3"
  value="$4"
  temp_file="$(mktemp)"

  if [[ "$section" -eq '' ]]; then
    split -p "^\[" "$file" "${temp_file}_"

    echo "sed \"s/^${key}.*$/${key} = ${value}/\" \"${temp_file}_aa\""
    (
      sed "s/^${key}.*$/${key} = ${value}/" "${temp_file}_aa"
      rm "${temp_file}_aa"
      cat "${temp_file}_"*
    ) > "$file"
  elif grep -E "^\\[${section}]\$" "$file" > /dev/null; then
    cp "$file" "$temp_file"
    split -p "\[${section}\]" "$temp_file" "${temp_file}_"

    (
      cat "${temp_file}_aa"
      sed "s/^${key}.*$/${key} = ${value}/" "${temp_file}_ab"
    ) > "$file"
  else
    (
      echo
      echo "[${section}]"
      echo "${key} = ${value}"
    ) >> "$file"
  fi
}

usage() {
  cat <<END_OF_USAGE 1>&2

Usage: $0 -f CONFIG_FILE [-i IP_ADDRESS_RANGE] [-p PORT] <start> <end>

    -f CONFIG_FILE       A path to the config.toml file to change, with
                         \"%\" replaced by the node id.
    -i IP_ADDRESS_RANGE  An IP Address start for nodes, which replaces
                         \"%\" with the node id. Default \"10.254.254.%\".
    -p PORT              The port instances are listening to, default 26656.

END_OF_USAGE
  exit 1
}

ip_range=10.254.254.%
port=26656
file=""
while getopts ":i:p:f:" opt; do
    case "${opt}" in
        i)  ip_range="${OPTARG}"
            ;;
        p)  port="${OPTARG}"
            [[ "$port" =~ ^[0-9]+$ ]] || usage
            ;;
        f)  file="${OPTARG}"
            ;;
        *)  usage
            ;;
    esac
done
shift $((OPTIND-1))

[ "$file" ] || usage

for node in $(seq "$1" "$2"); do
  config_toml_path=${file//%/$node}
  peer_ids=$(seq "$1" "$2" | grep -v "$node")
  peers=$(for peer in $peer_ids; do
    node_id=$(get_node_id "$peer")
    ip_address=${ip_range//%/$peer}

    printf '%s' "$node_id@$ip_address:$port,"
  done | sed 's/,$//')

#  update_toml_key "$config_toml_path" p2p persistent-peers "\"$peers\""
  update_toml_key "$config_toml_path" '' moniker "\"omni-tendermint-${node}\""
done
