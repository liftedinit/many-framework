GIT_ROOT="$BATS_TEST_DIRNAME/../../"

load '../test_helper/load'

function setup() {
    mkdir "$BATS_TEST_ROOTDIR"

    skip_if_missing_background_utilities

    (
      cd "$GIT_ROOT"
      cargo build --all-features
    )
}

function teardown() {
    stop_background_run
}

@test "$SUITE: IdStore store and get" {
    run_in_background "$GIT_ROOT/target/debug/many-ledger" \
          -v \
          --clean \
          --persistent "$(mktemp -d)" \
          --state "$GIT_ROOT/staging/ledger_state.json5" \
          --pem "$(pem 0)" \
          "--disable-webauthn-only-for-testing" # Disable WebAuthn check for this test
    wait_for_background_output "Running accept thread"

    identity_hex=$(identity_hex 1)
    cred_id=$(cred_id)
    key2cose=$(key2cose 1)

    many_message --id=0 idstore.store "{0: 10000_1(h'"${identity_hex}"'), 1: h'"${cred_id}"', 2: h'"${key2cose}"'}"
    assert_output '{0: ["abandon", "again"]}'

    many_message --id=0 idstore.getFromRecallPhrase "$output"
    assert_output --partial "0: h'"${cred_id,,}"'"
    assert_output --partial "1: h'"${key2cose}"'"

    many_message --id=0 idstore.getFromAddress '{0: "'$(identity 1)'"}'
    assert_output --partial "0: h'"${cred_id,,}"'"
    assert_output --partial "1: h'"${key2cose}"'"

    stop_background_run
}

@test "$SUITE: IdStore store deny non-webauthn" {
    run_in_background "$GIT_ROOT/target/debug/many-ledger" \
          -v \
          --clean \
          --persistent "$(mktemp -d)" \
          --state "$GIT_ROOT/staging/ledger_state.json5" \
          --pem "$(pem 0)"
    wait_for_background_output "Running accept thread"

    many_message --error --id=0 idstore.store "{0: 10000_1(h'"$(identity_hex 1)"'), 1: h'"$(cred_id)"', 2: h'"$(key2cose 1)"'}"
    assert_output --partial "Non-WebAuthn request denied for endpoint"

    stop_background_run
}