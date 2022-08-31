PEM_ROOT="$(mktemp -d)"

# Do not rename this function `ledger`.
# It clashes with the call to the `ledger` binary on CI
function call_ledger() {
    local pem_arg
    local port

    while (( $# > 0 )); do
      case "$1" in
        --pem=*) pem_arg="--pem=$(pem "${1#--pem=}")"; shift ;;
        --port=*) port=${1#--port=}; shift ;;
        --) shift; break ;;
        *) break ;;
      esac
    done

    local ledgercmd
    [[ "$CI" == "true" ]]\
      && ledgercmd="ledger" \
      || ledgercmd="$GIT_ROOT/target/debug/ledger"

    echo "${ledgercmd}" "$pem_arg" "http://localhost:${port}/" "$@" >&2
    run "${ledgercmd}" "$pem_arg" "http://localhost:${port}/" "$@"
}

function check_consistency() {
    local pem
    local expected_balance
    local id

    while (( $# > 0 )); do
      case "$1" in
        --pem=*) pem=${1#--pem=}; shift ;;
        --balance=*) expected_balance=${1#--balance=}; shift;;
        --id=*) id=${1#--id=}; shift ;;
        --) shift; break ;;
        *) break ;;
      esac
    done

    for port in "$@"; do
        call_ledger --pem="$pem" --port="$port" balance "$id"
        assert_output --partial "$expected_balance MFX "
    done
}

function account_create() {
    local pem_arg
    while (( $# > 0 )); do
      case "$1" in
        --pem=*) pem_arg="${1}"; shift ;;
        --) shift; break ;;
        *) break ;;
      esac
    done

    account_id="$(many_message "$pem_arg" account.create "$@" | grep -o "h'[0-9a-z]*'" | grep -oE "[0-9a-z][0-9a-z]+")"
    account_many_id=$(many id "$account_id")
    assert [ "${account_many_id::1}" = "m" ]  # Check the account ID starts with an "m"
    assert [ ${#account_many_id} -eq 55 ]     # Check the account ID has the right length
    echo "${account_many_id}"
}
