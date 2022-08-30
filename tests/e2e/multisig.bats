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

@test "$SUITE: Ledger shows a balance and can send tokens" {
    check_consistency "$(pem 1)" $START_BALANCE "$(pem 1)" 0

    call_ledger "$(pem 1)" 0 send "$(identity 3)" 1000 MFX
    check_consistency "$(pem 3)" 1000 "$(pem 3)" 0
    check_consistency "$(pem 1)" $((START_BALANCE - 1000)) "$(pem 1)" 0

    check_consistency "$(pem 2)" $START_BALANCE "$(pem 2)" 0
}

@test "$SUITE: Ledger can do account creation and multisig transactions" {
    local account_id
    local tx_id

    check_consistency "$(pem 1)" $START_BALANCE "$(pem 1)" 0
    account_id=$(account_create "$(pem 1)" '{ 1: { "'"$(identity 2)"'": ["canMultisigApprove"] }, 2: [[1, { 0: 2 }]] }')

    call_ledger "$(pem 1)" 0 send "$account_id" 1000000 MFX
    check_consistency "$(pem 1)" 1000000 "$account_id" 0

    call_ledger "$(pem 1)" 0 multisig submit "$account_id" send "$(identity 3)" 100 MFX
    tx_id=$(echo "$output" | grep -oE "[0-9a-f]+$")
    # Cannot execute if not approved.
    call_ledger "$(pem 1)" 0 multisig execute "$tx_id"
    assert_output --partial "This transaction cannot be executed yet."

    call_ledger "$(pem 2)" 0 multisig approve "$tx_id"

    # Cannot execute if not submitted.
    call_ledger "$(pem 2)" 0 multisig execute "$tx_id"
    assert_output --partial "This transaction cannot be executed yet."

    call_ledger "$(pem 1)" 0 multisig execute "$tx_id"

    check_consistency "$(pem 1)" 999900 "$account_id" 0
    check_consistency "$(pem 3)" 100 "$(pem 3)" 0
}

@test "$SUITE: can revoke" {
    local account_id
    local tx_id

    check_consistency "$(pem 1)" $START_BALANCE "$(pem 1)" 0
    account_id=$(account_create "$(pem 1)" '{ 1: { "'"$(identity 2)"'": ["canMultisigApprove"] }, 2: [[1, { 0: 2 }]] }')

    call_ledger "$(pem 1)" 0 send "$account_id" 1000000 MFX
    check_consistency "$(pem 1)" 1000000 "$account_id" 0

    call_ledger "$(pem 1)" 0 multisig submit "$account_id" send "$(identity 3)" 100 MFX
    tx_id=$(echo "$output" | grep -oE "[0-9a-f]+$")

    call_ledger "$(pem 2)" 0 multisig approve "$tx_id"
    call_ledger "$(pem 1)" 0 multisig revoke "$tx_id"

    call_ledger "$(pem 1)" 0 multisig execute "$tx_id"
    assert_output --partial "This transaction cannot be executed yet."

    call_ledger "$(pem 1)" 0 multisig approve "$tx_id"
    call_ledger "$(pem 2)" 0 multisig revoke "$tx_id"
    call_ledger "$(pem 1)" 0 multisig execute "$tx_id"
    assert_output --partial "This transaction cannot be executed yet."

    call_ledger "$(pem 2)" 0 multisig approve "$tx_id"
    call_ledger "$(pem 1)" 0 multisig execute "$tx_id"

    check_consistency "$(pem 3)" 100 "$(pem 3)" 0
}
