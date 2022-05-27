function pem() {
    [ -f "$PEM_ROOT/id-$1.pem" ] || openssl genpkey -algorithm Ed25519 -out "$PEM_ROOT/id-$1.pem" >/dev/null
    echo "$PEM_ROOT/id-$1.pem"
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

function account() {
    command many id mahukzwuwgt3porn6q4vq4xu3mwy5gyskhouryzbscq7wb2iow "$1"
}
