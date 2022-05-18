GIT_ROOT="$BATS_TEST_DIRNAME/../../"
START_BALANCE=100000000000
MFX_ADDRESS=mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz

load '../test_helper/load'

function setup() {
    mkdir "$BATS_TEST_ROOTDIR"

    skip_if_missing_background_utilities

    (
      cd "$GIT_ROOT"
      cargo build --all-features
    )

    run_in_background "$GIT_ROOT/target/debug/many-ledger" \
          -v \
          --clean \
          --persistent "$(mktemp -d)" \
          --state "$GIT_ROOT/staging/ledger_state.json" \
          --pem "$(pem 0)" \
          "--balance-only-for-testing=$(identity 1):$START_BALANCE:$MFX_ADDRESS" \
          "--balance-only-for-testing=$(identity 2):$START_BALANCE:$MFX_ADDRESS"

    wait_for_background_output "Running accept thread"
}

function teardown() {
    stop_background_run
}

@test "$SUITE: Ledger shows a balance and can send tokens" {
    ledger --id=1 balance
    assert_output --partial "$START_BALANCE MFX"

    ledger --id=1 send "$(identity 3)" 1000 MFX

    ledger --id=3 balance
    assert_output --partial "1000 MFX"

    ledger --id=1 balance
    assert_output --partial "$((START_BALANCE - 1000)) MFX"

    ledger --id=2 balance
    assert_output --partial "$START_BALANCE MFX"
}

@test "$SUITE: Ledger can do account creation and multisig transactions" {
    local account_id
    local tx_id

    ledger --id=1 balance
    assert_output --partial "$START_BALANCE MFX"

    account_id=$(account_create --id=1 '{ 1: { "'"$(identity 2)"'": ["canMultisigApprove"] }, 2: [[1, { 0: 2 }]] }')

    ledger --id=1 send "$account_id" 1000000 MFX
    ledger --id=1 balance "$account_id"
    assert_output --partial "1000000 MFX"

    ledger --id=1 multisig submit "$account_id" send "$(identity 3)" 100 MFX
    tx_id=$(echo "$output" | grep -oE "[0-9a-f]+$")
    # Cannot execute if not approved.
    ledger_error --id=1 multisig execute "$tx_id"

    ledger --id=2 multisig approve "$tx_id"

    # Cannot execute if not submitted.
    ledger_error --id=2 multisig execute "$tx_id"

    ledger --id=1 multisig execute "$tx_id"

    ledger --id=1 balance "$account_id"
    assert_output --partial "999900 MFX"

    ledger --id=3 balance
    assert_output --partial "100 MFX"
}

@test "$SUITE: can revoke" {
    local account_id
    local tx_id

    ledger --id=1 balance
    assert_output --partial "$START_BALANCE MFX"

    account_id=$(account_create --id=1 '{ 1: { "'"$(identity 2)"'": ["canMultisigApprove"] }, 2: [[1, { 0: 2 }]] }')

    ledger --id=1 send "$account_id" 1000000 MFX
    ledger --id=1 balance "$account_id"
    assert_output --partial "1000000 MFX"

    ledger --id=1 multisig submit "$account_id" send "$(identity 3)" 100 MFX
    tx_id=$(echo "$output" | grep -oE "[0-9a-f]+$")

    ledger --id=2 multisig approve "$tx_id"
    ledger --id=1 multisig revoke "$tx_id"

    ledger_error --id=1 multisig execute "$tx_id"

    ledger --id=1 multisig approve "$tx_id"
    ledger --id=2 multisig revoke "$tx_id"
    ledger_error --id=1 multisig execute "$tx_id"

    ledger --id=2 multisig approve "$tx_id"
    ledger --id=1 multisig execute "$tx_id"

    ledger --id=3 balance
    assert_output --partial "100 MFX"
}
