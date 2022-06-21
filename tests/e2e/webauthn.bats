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
    assert_output --partial "0: h'$(echo $cred_id | tr A-Z a-z)'"
    assert_output --partial "1: h'${key2cose}'"
}

@test "$SUITE: IdStore store deny non-webauthn" {
    start_ledger --pem "$(pem 0)"

    many_message --error --id=0 idstore.store "{0: 10000_1(h'$(identity_hex 1)'), 1: h'$(cred_id)', 2: h'$(key2cose 1)'}"
    assert_output --partial "Non-WebAuthn request denied for endpoint"
}

@test "$SUITE: IdStore export works" {
    which jq || skip "'jq' needs to be installed for this test."
    local ledger_db
    local state
    ledger_db="$(mktemp -d)"
    state="$GIT_ROOT/tests/e2e/webauthn_state.json"

    start_ledger \
        "--persistent=$ledger_db" \
        "--state=$state" \
        --pem "$(pem 0)" \
        --disable-webauthn-only-for-testing # Disable WebAuthn check for this test

    identity_hex=$(identity_hex 1)
    cred_id=$(cred_id)
    key2cose=$(key2cose 1)

    many_message --id=0 idstore.store "{0: 10000_1(h'${identity_hex}'), 1: h'${cred_id}', 2: h'${key2cose}'}"
    assert_output '{0: ["abandon", "again"]}'

    # Stop and regenesis.
    stop_background_run

    # Export to a temp file.
    local export_file
    export_file="$(mktemp)"
    "$GIT_ROOT/target/debug/idstore-export" "$ledger_db" > "$export_file"
    local import_file
    import_file="$(mktemp)"
    jq -s '.[0] * .[1]' "$state" "$export_file" > "$import_file"

    cat $import_file >&2

    start_ledger \
        --persistent="$ledger_db" \
        --state="$import_file" \
        --pem "$(pem 0)" \
        --disable-webauthn-only-for-testing # Disable WebAuthn check for this test

    # Continue the test.
    many_message --id=0 idstore.store "{0: 10000_1(h'${identity_hex}'), 1: h'${cred_id}', 2: h'${key2cose}'}"
    assert_output '{0: ["abandon", "asset"]}'

    many_message --id=0 idstore.getFromRecallPhrase "$output"
    assert_output --partial "0: h'$(echo $cred_id | tr A-Z a-z)'"
    assert_output --partial "1: h'${key2cose}'"

    many_message --id=0 idstore.getFromAddress '{0: "'$(identity 1)'"}'
    assert_output --partial "0: h'$(echo $cred_id | tr A-Z a-z)'"
    assert_output --partial "1: h'${key2cose}'"
}
