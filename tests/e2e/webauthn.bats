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
    start_ledger --pem "$(pem 0)" \
                 --disable-webauthn-only-for-testing # Disable WebAuthn check for this test

    identity_hex=$(identity_hex 1)
    cred_id=$(cred_id)
    key2cose=$(key2cose 1)

    many_message --id=0 idstore.store "{0: 10000_1(h'${identity_hex}'), 1: h'${cred_id}', 2: h'${key2cose}'}"
    assert_output '{0: ["abandon", "again"]}'

    many_message --id=0 idstore.getFromRecallPhrase "$output"
    assert_output --partial "0: h'$(echo $cred_id | tr A-Z a-z)'"
    assert_output --partial "1: h'${key2cose}'"

    many_message --id=0 idstore.getFromAddress '{0: "'$(identity 1)'"}'
    assert_output --partial "0: h'"$(echo $cred_id | tr A-Z a-z)"'"
    assert_output --partial "1: h'"${key2cose}"'"
}

@test "$SUITE: IdStore store deny non-webauthn" {
    start_ledger --pem "$(pem 0)"

    many_message --error --id=0 idstore.store "{0: 10000_1(h'$(identity_hex 1)'), 1: h'$(cred_id)', 2: h'$(key2cose 1)'}"
    assert_output --partial "Non-WebAuthn request denied for endpoint"
}

@test "$SUITE: IdStore export works" {
    which jq || skip "'jq' needs to be installed for this test."

    start_ledger --pem "$(pem 0)" \
          --disable-webauthn-only-for-testing # Disable WebAuthn check for this test

    identity_hex=$(identity_hex 1)
    cred_id=$(cred_id)
    key2cose=$(key2cose 1)

    many_message --id=0 idstore.store "{0: 10000_1(h'${identity_hex}'), 1: h'${cred_id}', 2: h'${key2cose}'}"
    assert_output '{0: ["abandon", "again"]}'

    # Stop and regenesis.
    stop_background_run

    # Export to a temp file.
    local EXPORT_FILE
    EXPORT_FILE="$(mktemp)"
    "$GIT_ROOT/target/debug/idstore-export"  >
}
