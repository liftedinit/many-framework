function pem() {
    [ -f "$PEM_ROOT/id-$1.pem" ] || openssl genpkey -algorithm Ed25519 -out "$PEM_ROOT/id-$1.pem" >/dev/null
    echo "$PEM_ROOT/id-$1.pem"
}

function many_message() {
    local pem_arg
    if [[ "$1" == "--id="* ]]; then
        pem_arg="--pem=$(pem ${1#--id=})"
        shift
    fi
    run command many message --server http://localhost:8000 "$pem_arg" "$@"
    [ "$status" -eq 0 ]
}

function identity() {
    command many id "$(pem $1)"
}
