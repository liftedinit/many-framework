function timeout() { perl -e 'alarm shift; exec @ARGV' "$@"; }

GIT_ROOT="$BATS_TEST_DIRNAME/../../"
START_BALANCE=100000000000
LEDGER_IDENTITY_PEM="$GIT_ROOT/tests/id1.pem"
FBT_ADDRESS=mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz

function setup() {
    load '../test_helper/load'

    skip_if_missing_background_utilities

    (
      cd "$GIT_ROOT"
      cargo build --all-features
    )

    which many >/dev/null

    run_in_background "$GIT_ROOT/target/debug/many-ledger" \
          -v \
          --clean \
          --persistent "$(mktemp -d)" \
          --state "$GIT_ROOT/staging/ledger_state.json" \
          --pem "$LEDGER_IDENTITY_PEM" \
          "--balance-only-for-testing=$(many id "$GIT_ROOT/tests/id1.pem"):$START_BALANCE:$FBT_ADDRESS" \
          "--balance-only-for-testing=$(many id "$GIT_ROOT/tests/id2.pem"):$START_BALANCE:$FBT_ADDRESS"

    wait_for_background_output "Running accept thread"
}

function teardown() {
    stop_background_run
}

function ledger() {
    local pem="$1"
    shift
    run ../../target/debug/ledger --pem "../${pem}" "http://localhost:8000/" "$@"
}

@test "Ledger shows a balance and can send tokens" {
    ledger id1.pem balance
    assert_output --partial "$START_BALANCE FBT"

    ledger id1.pem send "$(many id "$GIT_ROOT/tests/id3.pem")" 1000 FBT

    ledger id3.pem balance
    assert_output --partial "1000 FBT"

    ledger id1.pem balance
    assert_output --partial "$((START_BALANCE - 1000)) FBT"

    ledger id2.pem balance
    assert_output --partial "$START_BALANCE FBT"
}

@test "Ledger can do account creation and multisig" {
    ledger id1.pem balance
    assert_output --partial "$START_BALANCE FBT"

    ledger id1.pem send "$(many id "$GIT_ROOT/tests/id3.pem")" 1000 FBT

    ledger id3.pem balance
    assert_output --partial "1000 FBT"

    ledger id1.pem balance
    assert_output --partial "$((START_BALANCE - 1000)) FBT"

    ledger id2.pem balance
    assert_output --partial "$START_BALANCE FBT"
}
