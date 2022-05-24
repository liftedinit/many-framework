PEM_ROOT="$(mktemp -d)"

function ledger() {
    local pem_arg
    if [[ "$1" == "--id="* ]]; then
        pem_arg="--pem=$(pem ${1#--id=})"
        shift
    fi
    run ../../target/debug/ledger "$pem_arg" "http://localhost:8000/" "$@"
    [ "$status" -eq 0 ]
}

function ledger_error() {
    local pem_arg
    if [[ "$1" == "--id="* ]]; then
        pem_arg="--pem=$(pem ${1#--id=})"
        shift
    fi
    run ../../target/debug/ledger "$pem_arg" "http://localhost:8000/" "$@"
    [ "$status" -ne 0 ]
}

function account_create() {
    local id_arg

    if [[ "$1" == "--id="* ]]; then
        id_arg="$1"
        shift
    fi
    many_message "$id_arg" account.create "$@"
    account_id="$(echo "$output" | grep -o "h'[0-9a-z]*'" | grep -oE "[0-9a-z][0-9a-z]+")"
    command many id "$account_id"
}
