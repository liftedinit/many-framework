PEM_ROOT="$(mktemp -d)"

# Do not rename this function `kvstore`.
# It clashes with the call to the `kvstore` binary on CI
function call_kvstore() {
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

    local kvstorecmd
    [[ "$CI" == "true" ]]\
      && kvstorecmd="kvstore" \
      || kvstorecmd="$GIT_ROOT/target/debug/kvstore"

    echo "${kvstorecmd} $pem_arg http://localhost:${port}/ $*" >&2
    run bash -c "${kvstorecmd} $pem_arg http://localhost:${port}/ $*"
}

function check_consistency() {
    local pem_arg
    local key
    local expected_value

    while (( $# > 0 )); do
      case "$1" in
        --pem=*) pem_arg=${1}; shift ;;
        --key=*) key=${1#--key=}; shift ;;
        --value=*) expected_value=${1#--value=}; shift;;
        --) shift; break ;;
        *) break ;;
      esac
    done

    for port in "$@"; do
        call_kvstore --port="$port" "$pem_arg" get "$key"
        assert_output --partial "$expected_value"
    done
}
