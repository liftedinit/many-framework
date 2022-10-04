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

    echo "${ledgercmd} $pem_arg http://localhost:${port}/ $*" >&2
    run bash -c "${ledgercmd} $pem_arg http://localhost:${port}/ $*"
}

function check_consistency() {
    local pem_arg
    local expected_balance
    local id

    while (( $# > 0 )); do
      case "$1" in
        --pem=*) pem_arg=${1}; shift ;;
        --balance=*) expected_balance=${1#--balance=}; shift;;
        --id=*) id=${1#--id=}; shift ;;
        --) shift; break ;;
        *) break ;;
      esac
    done

    for port in "$@"; do
        call_ledger "--port=$port" "$pem_arg" balance "$id"
        assert_output --partial "$expected_balance MFX "
    done
}
