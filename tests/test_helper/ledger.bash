PEM_ROOT="$(mktemp -d)"

function ledger() {
    local pem_arg
    local error
    local check_balance

    while (( $# > 0 )); do
      case "$1" in
        --id=*) pem_arg="--pem=$(pem "${1#--id=}")"; shift ;;
        -e|--error) error=1; shift ;;
        --balance=*) check_balance="${1#--balance=}"; shift ;;
        --) shift; break ;;
        *) break ;;
      esac
    done

    echo ../../target/debug/ledger "$pem_arg" "http://localhost:8000/" "$@" >&2
    run ../../target/debug/ledger "$pem_arg" "http://localhost:8000/" "$@"
    if [ "$error" ]; then
      [ "$status" -ne 0 ]
    else
      [ "$status" -eq 0 ]
      if [ "$check_balance" ]; then
        assert_output --partial "${check_balance} MFX"
      fi
    fi
}

function account_create() {
    local id_arg

    while (( $# > 0 )); do
      case "$1" in
        --id=*) id_arg="$1"; shift ;;
        --) shift; break ;;
        *) break ;;
      esac
    done

    many_message "$id_arg" account.create "$@"
    account_id="$(echo "$output" | grep -o "h'[0-9a-z]*'" | grep -oE "[0-9a-z][0-9a-z]+")"
    command many id "$account_id"
}
