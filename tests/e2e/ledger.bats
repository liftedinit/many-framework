GIT_ROOT="$BATS_TEST_DIRNAME/../../"
START_BALANCE=100000000000
MFX_ADDRESS=mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz

load '../test_helper/load'

function setup() {
    mkdir "$BATS_TEST_ROOTDIR"

    skip_if_missing_background_utilities

#    (
#      cd "$GIT_ROOT"
#      cargo build --all-features
#    )

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

@test "$SUITE: Ledger can send tokens on behalf of an account" {
    account_id=$(account_create --id=1 '{ 1: { "'"$(identity 2)"'": ["canLedgerTransact"] }, 2: [0] }')
    ledger --id=1 send "$account_id" 1000000 MFX
    ledger --balance=1000000 --id=1 balance "$account_id"

    ledger --id=1 send --account="$account_id" "$(identity 4)" 2000 MFX
    ledger --id=4 --balance=2000 balance
    ledger --id=1 --balance=998000 balance "$account_id"

    ledger --id=2 send --account="$account_id" "$(identity 4)" 2000 MFX
    ledger --id=4 --balance=4000 balance
    ledger --id=1 --balance=996000 balance "$account_id"

    ledger --error --id=4 send --account="$account_id" "$(identity 4)" 2000 MFX
}
