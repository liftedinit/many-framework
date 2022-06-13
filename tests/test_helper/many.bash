function pem() {
    [ -f "$PEM_ROOT/id-$1.pem" ] || openssl genpkey -algorithm Ed25519 -out "$PEM_ROOT/id-$1.pem" >/dev/null
    echo "$PEM_ROOT/id-$1.pem"
}

# Print the X-coord of an Ed25519 public key
function ed25519_x_coord() {
    openssl pkey -in "$(pem "$1")" -text_pub -noout | grep "    " | awk '{printf("%s ",$0)} END { printf "\n" }' | sed '$s/\s\+//g' | tr -d ':'
}

# Return a CBOR encoded CoseKey created from a Ed25519 key.
# Requires https://github.com/cabo/cbor-diag in your $PATH
function key2cose() {
  echo "{1: 1, 2: h'"$(identity_hex "$1")"', 3: -8, 4: [2], -1: 6, -2: h'"$(ed25519_x_coord "$1")"'}" | diag2cbor.rb | xxd -p -c 10000
}

# Return 16 bytes of random data
function cred_id() {
  hexdump -vn16 -e'4/4 "%08X" 1 "\n"' /dev/urandom
}

function many_message() {
    local pem_arg
    local error

    while (( $# > 0 )); do
      case "$1" in
        --id=*) pem_arg="--pem=$(pem ${1#--id=})"; shift ;;
        -e|--error) error=1; shift ;;
        --) shift; break ;;
        *) break ;;
      esac
    done

    run command many message --server http://localhost:8000 "$pem_arg" "$@"
    if [ "$error" ]; then
      [ "$status" -ne 0 ]
    else
      [ "$status" -eq 0 ]
    fi
}

function identity() {
    command many id "$(pem "$1")"
}

function identity_hex() {
    command many id $(many id "$(pem "$1")")
}

function account() {
    command many id mahukzwuwgt3porn6q4vq4xu3mwy5gyskhouryzbscq7wb2iow "$1"
}
