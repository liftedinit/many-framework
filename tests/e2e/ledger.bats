GIT_ROOT="$BATS_TEST_DIRNAME/../../"
START_BALANCE=100000000000
MFX_ADDRESS=mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz

load '../test_helper/load'

function setup() {
    mkdir "$BATS_TEST_ROOTDIR"

    skip_if_missing_background_utilities

    if ! [ $CI ]; then
        (
          cd "$GIT_ROOT"
          cargo build --all-features
        )
    fi

    run_in_background "$GIT_ROOT/target/debug/many-ledger" \
          -v \
          --clean \
          --persistent "$(mktemp -d)" \
          --state "$GIT_ROOT/staging/ledger_state.json5" \
          --pem "$(pem 0)" \
          "--balance-only-for-testing=$(identity 1):$START_BALANCE:$MFX_ADDRESS" \
          "--balance-only-for-testing=$(identity 2):$START_BALANCE:$MFX_ADDRESS"

    wait_for_background_output "Running accept thread"
}

function teardown() {
    stop_background_run
}

@test "$SUITE: ledger can send tokens on behalf of an account" {
    account_id=$(account_create "$(pem 1)" '{ 1: { "'"$(identity 2)"'": ["canLedgerTransact"] }, 2: [0] }')
    call_ledger "$(pem 1)" 0 send "$account_id" 1000000 MFX
    check_consistency "$(pem 1)" 1000000 "$account_id" 0

    call_ledger "$(pem 1)" 0 send --account="$account_id" "$(identity 4)" 2000 MFX
    check_consistency "$(pem 4)" 2000 "$(pem 4)" 0
    check_consistency "$(pem 1)" 998000 "$account_id" 0

    call_ledger "$(pem 2)" 0 send --account="$account_id" "$(identity 4)" 2000 MFX
    check_consistency "$(pem 4)" 4000 "$(pem 4)" 0
    check_consistency "$(pem 1)" 996000 "$account_id" 0

    call_ledger "$(pem 4)" 0 send --account="$account_id" "$(identity 4)" 2000 MFX
    assert_output --partial "Sender needs role 'canLedgerTransact' to perform this operation."
}
