PEM_ROOT="$(mktemp -d)"

# Do not rename this function `ledger`.
# It clashes with the call to the `ledger` binary on CI
function call_ledger() {
    local pem="$1"
    local port="$2"
    shift 2

    local ledgercmd
    [[ "$CI" == "true" ]]\
      && ledgercmd="ledger" \
      || ledgercmd="$GIT_ROOT/target/debug/ledger"

    echo "${ledgercmd}" --pem "${pem}" "http://localhost:$((port + 8000))/" "$@" >&2
    run "${ledgercmd}" --pem "${pem}" "http://localhost:$((port + 8000))/" "$@"
}

function check_consistency() {
    local pem="$1"
    local expected_balance="$2"
    local id_arg="$3"
    shift 3

    for port in "$@"; do
        call_ledger "$pem" "$port" balance "$id_arg"
        assert_output --partial "$expected_balance MFX "
    done
}

function account_create() {
    local pem="$1"
    shift

    account_id="$(many_message "$pem" account.create "$@" | grep -o "h'[0-9a-z]*'" | grep -oE "[0-9a-z][0-9a-z]+")"
    account_many_id=$(many id "$account_id")
    assert [ "${account_many_id::1}" = "m" ]  # Check the account ID starts with an "m"
    assert [ ${#account_many_id} -eq 55 ]     # Check the account ID has the right length
    echo "${account_many_id}"
}
