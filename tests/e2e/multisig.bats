function timeout() { perl -e 'alarm shift; exec @ARGV' "$@"; }

PEM_ROOT="$(mktemp -d)"
GIT_ROOT="$BATS_TEST_DIRNAME/../../"
START_BALANCE=100000000000
FBT_ADDRESS=mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz

#function setup() {
#    load '../test_helper/load'
#
#    skip_if_missing_background_utilities
#
#    (
#      cd "$GIT_ROOT"
#      cargo build --all-features
#    )
#
#    run_in_background "$GIT_ROOT/target/debug/many-ledger" \
#          -v \
#          --clean \
#          --persistent "$(mktemp -d)" \
#          --state "$GIT_ROOT/staging/ledger_state.json" \
#          --pem "$(pem 0)" \
#          "--balance-only-for-testing=$(identity 1):$START_BALANCE:$FBT_ADDRESS" \
#          "--balance-only-for-testing=$(identity 2):$START_BALANCE:$FBT_ADDRESS"
#
#    wait_for_background_output "Running accept thread"
#}
#
#function teardown() {
#    stop_background_run
#}

function pem() {
  [ -f "$PEM_ROOT/id-$1.pem" ] || openssl genpkey -algorithm Ed25519 -out "$PEM_ROOT/id-$1.pem" >/dev/null
  echo "$PEM_ROOT/id-$1.pem"
}

function ledger() {
    local pem_file
    pem_file="$(pem "$1")"
    shift
    run ../../target/debug/ledger --pem "$pem_file" "http://localhost:8000/" "$@"
}

function many() {
    local pem_arg
    if [ "$1" == "--id=*" ]; then
        pem_args="--pem=\"$(pem ${1#"--id="})\""
        shift
    fi
    run command many --server http://localhost:8000 "$pem_arg" "$@"
}

function identity() {
    command many id "$(pem $1)"
}

#@test "Ledger shows a balance and can send tokens" {
#    ledger --id=1 balance
#    assert_output --partial "$START_BALANCE FBT"
#
#    ledger --id=1 send "$(identity 3)" 1000 FBT
#
#    ledger --id=3 balance
#    assert_output --partial "1000 FBT"
#
#    ledger --id=1 balance
#    assert_output --partial "$((START_BALANCE - 1000)) FBT"
#
#    ledger --id=2 balance
#    assert_output --partial "$START_BALANCE FBT"
#}
#
#@test "Ledger can do account creation and multisig" {
#    local account_id
#
#    ledger --id=1 balance
#    assert_output --partial "$START_BALANCE FBT"
#
#    many 1 account.create '{ 1: { "'"$(identity 2)"'": ["canMultisigApprove"] }, 2: [[1, { 0: 2 }]] }'
#    account_id="$(echo "$OUTPUT" | grep -o "0: .*" | colrm 1 3)"
#    echo $account_id >&2
#    exit 1
#
#    ledger --id=1 send "$(identity 2)" 1000 FBT
#}

@test "a" {
    echo run_in_background "$GIT_ROOT/target/debug/many-ledger" \
          -v \
          --clean \
          --persistent "$(mktemp -d)" \
          --state "$GIT_ROOT/staging/ledger_state.json" \
          --pem "$(pem 0)" \
          "--balance-only-for-testing=$(identity 1):$START_BALANCE:$FBT_ADDRESS" \
          "--balance-only-for-testing=$(identity 2):$START_BALANCE:$FBT_ADDRESS"
  false
}
