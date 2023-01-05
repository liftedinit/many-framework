# e2e tests for the token feature set
# The Token Migration needs to be active for this feature set to be enabled.

GIT_ROOT="$BATS_TEST_DIRNAME/../../../"
MFX_ADDRESS=mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz

load '../../test_helper/load'
load '../../test_helper/ledger'

function setup() {
    mkdir "$BATS_TEST_ROOTDIR"

    skip_if_missing_background_utilities

    (
      cd "$GIT_ROOT"
      cargo build --features migration_testing
    )

    echo '
    { "migrations": [
      {
        "name": "Account Count Data Attribute",
        "block_height": 0,
        "disabled": true
      },
      {
        "name": "Block 9400",
        "block_height": 0,
        "disabled": true
      },
      {
        "name": "Dummy Hotfix",
        "block_height": 0,
        "disabled": true
      },
      {
        "name": "Memo Migration",
        "block_height": 0,
        "disabled": true
      },
      {
        "name": "Token Migration",
        "block_height": 0
      }
    ] }' > "$BATS_TEST_ROOTDIR/migrations.json"

    # Activating the Token Migration from block 0 will modify the ledger staging hash
    # The symbol metadata will be stored in the DB
    cp "$GIT_ROOT/staging/ledger_state.json5" "$BATS_TEST_ROOTDIR/ledger_state.json5"

    # Make `identity 1` the token identity
    sed -i 's/token_identity: ".*"/token_identity: "'"$(identity 1)"'"/' "$BATS_TEST_ROOTDIR/ledger_state.json5"

    # Use token identity subresource 0 as the first token symbol
    sed -i 's/token_next_subresource: 2/token_next_subresource: 0/' "$BATS_TEST_ROOTDIR/ledger_state.json5"

    # Skip hash check
    sed -i 's/hash/\/\/hash/' "$BATS_TEST_ROOTDIR/ledger_state.json5"

    start_ledger --state="$BATS_TEST_ROOTDIR/ledger_state.json5" \
        --pem "$(pem 0)" \
        --migrations-config "$BATS_TEST_ROOTDIR/migrations.json"
}

function teardown() {
    stop_background_run
}

@test "$SUITE: can create new token" {
    create_token --pem=1 --port=8000 \
        --initial-distribution "$(identity 1)" 1000 \
        --initial-distribution "$(identity 2)" 1000
    assert_output --regexp "total:.*(.*2000,.*)"
    assert_output --regexp "circulating:.*(.*2000,.*)"
}

@test "$SUITE: can create new token with memo" {
    create_token --pem=1 --port=8000 --ext_info_type="memo"
}

@test "$SUITE: can create new token with unicode logo" {
    create_token --pem=1 --port=8000 --ext_info_type="unicode"
}

@test "$SUITE: can create new token with image logo" {
    create_token --pem=1 --port=8000 --ext_info_type="image"
}

@test "$SUITE: can't create as anonymous" {
    create_token --error=anon --port=8000
}

@test "$SUITE: can't create as identity 2" {
    create_token --pem=2 --error=invalid_sender --port=8000
}

@test "$SUITE: can update token" {
    create_token --pem=1 --port=8000

    call_ledger --pem=1 --port=8000 token update --name "\"New name\"" \
        --ticker "LLT" \
        --decimals "42" \
        --memo "\"Update memo\"" \
        --owner "$(identity 2)" \
        "${SYMBOL}"

    call_ledger --port=8000 token info "${SYMBOL}"
    assert_output --partial "name: \"New name\""
    assert_output --partial "ticker: \"LLT\""
    assert_output --partial "decimals: 42"
    assert_output --regexp "owner:.*$(identity 2).*)"
}

@test "$SUITE: can't update as non-owner" {
    create_token --pem=1 --port=8000
    call_ledger --pem=2 --port=8000 token update --owner "$(identity 2)" "${SYMBOL}"
    assert_output --partial "Unauthorized to do this operation."
}

@test "$SUITE: can't update as anonymous" {
    create_token --pem=1 --port=8000
    call_ledger --port=8000 token update --owner "$(identity 2)" "${SYMBOL}"
    assert_output --partial "Invalid Identity; the sender cannot be anonymous."
}

@test "$SUITE: can add extended info (memo)" {
    create_token --pem=1 --port=8000
    call_ledger --pem=1 --port=8000 token add-ext-info "${SYMBOL}" memo "\"My memo\""
    call_ledger --port=8000 token info "${SYMBOL}"
    assert_output --partial "My memo"
}

@test "$SUITE: can add extended info (logo - unicode)" {
    create_token --pem=1 --port=8000
    call_ledger --pem=1 --port=8000 token add-ext-info "${SYMBOL}" logo unicode  "'∑'"
    call_ledger --port=8000 token info "${SYMBOL}"
    assert_output --partial "'∑'"
}

@test "$SUITE: can add extended info (logo - image)" {
    create_token --pem=1 --port=8000
    call_ledger --pem=1 --port=8000 token add-ext-info "${SYMBOL}" logo image "png" "\"hello\""
    call_ledger --port=8000 token info "${SYMBOL}"
    assert_output --partial "png"
    assert_output --regexp "binary: \[.*104,.*101,.*108,.*108,.*111,.*\]"
}

@test "$SUITE: can't add extended info as anonymous" {
    create_token --pem=1 --port=8000

    # Memo
    call_ledger --port=8000 token add-ext-info "${SYMBOL}" memo "\"My memo\""
    assert_output --partial "Invalid Identity; the sender cannot be anonymous."

    # Logo - unicode
    call_ledger --port=8000 token add-ext-info "${SYMBOL}" logo unicode  "'∑'"
    assert_output --partial "Invalid Identity; the sender cannot be anonymous."

    # Logo - image
    call_ledger --port=8000 token add-ext-info "${SYMBOL}" logo image "png" "\"hello\""
    assert_output --partial "Invalid Identity; the sender cannot be anonymous."
}

@test "$SUITE: can't add extended info as non-owner" {
    create_token --pem=1 --port=8000

    # Memo
    call_ledger --pem=2 --port=8000 token add-ext-info "${SYMBOL}" memo "\"My memo\""
    assert_output --partial "Unauthorized to do this operation."

    # Logo - unicode
    call_ledger --pem=2 --port=8000 token add-ext-info "${SYMBOL}" logo unicode  "'∑'"
    assert_output --partial "Unauthorized to do this operation."

    # Logo - image
    call_ledger --pem=2 --port=8000 token add-ext-info "${SYMBOL}" logo image "png" "\"hello\""
    assert_output --partial "Unauthorized to do this operation."
}

@test "$SUITE: can remove extended info (memo)" {
    create_token --pem=1 --port=8000 --ext_info_type="memo"
    # Remove memo
    call_ledger --pem=1 --port=8000 token remove-ext-info "${SYMBOL}" 0
    refute_output --partial "Some memo"
}

@test "$SUITE: can remove extended info (logo - unicode)" {
    create_token --pem=1 --port=8000 --ext_info_type="unicode"
    # Remove logo
    call_ledger --pem=1 --port=8000 token remove-ext-info "${SYMBOL}" 1
    refute_output --partial "'∑'"
}

@test "$SUITE: can remove extended info (logo - image)" {
    create_token --pem=1 --port=8000 --ext_info_type="image"
    # Remove logo
    call_ledger --pem=1 --port=8000 token remove-ext-info "${SYMBOL}" 1
    refute_output --partial "png"
    refute_output --regexp "binary: \[.*104,.*101,.*108,.*108,.*111,.*\]"
}

@test "$SUITE: can't remove extended info as non-owner" {
    create_token --pem=1 --port=8000
    # Memo. We don't care that the token doesn't have one
    call_ledger --pem=2 --port=8000 token remove-ext-info "${SYMBOL}" 0
    assert_output --partial "Unauthorized to do this operation."
    # Logo. We don't care that the token doesn't have one
    call_ledger --pem=2 --port=8000 token remove-ext-info "${SYMBOL}" 1
    assert_output --partial "Unauthorized to do this operation."
}

@test "$SUITE: can't remove extended info as anonymous" {
    create_token --pem=1 --port=8000
    # Memo. We don't care that the token doesn't have one
    call_ledger --port=8000 token remove-ext-info "${SYMBOL}" 0
    assert_output --partial "Invalid Identity; the sender cannot be anonymous."
}

@test "$SUITE: can update token, token owner is account, caller is account owner" {
    token_account --perm="canTokensUpdate"
    call_ledger --pem=1 --port=8000 token update --name "\"New name\"" "${SYMBOL}"
    call_ledger --port=8000 token info "${SYMBOL}"
    assert_output --partial "name: \"New name\""
}

@test "$SUITE: can update token, token owner is account, caller have update permission" {
    token_account --perm="canTokensUpdate"
    call_ledger --pem=2 --port=8000 token update --name "\"New name\"" "${SYMBOL}"
    call_ledger --port=8000 token info "${SYMBOL}"
    assert_output --partial "name: \"New name\""
}

@test "$SUITE: can't update token, token owner is account, caller doesn't have update permission" {
    token_account --perm="canTokensUpdate"
    call_ledger --pem=3 --port=8000 token update --name "\"New name\"" "${SYMBOL}"
    assert_output --partial "Sender needs role 'canTokensUpdate' to perform this operation."
}

@test "$SUITE: can add extended info (memo), token owner is account, caller is account owner" {
    token_account --perm="canTokensAddExtendedInfo" --ext_info_type="memo"
    call_ledger --pem=1 --port=8000 token add-ext-info "${SYMBOL}" memo "\"My memo\""
    call_ledger --port=8000 token info "${SYMBOL}"
    assert_output --partial "My memo"
}

@test "$SUITE: can add extended info (logo - unicode), token owner is account, caller is account owner" {
    token_account --perm="canTokensAddExtendedInfo" --ext_info_type="unicode"
    call_ledger --pem=1 --port=8000 token add-ext-info "${SYMBOL}" logo unicode "'∑'"
    call_ledger --port=8000 token info "${SYMBOL}"
    assert_output --partial "'∑'"
}

@test "$SUITE: can add extended info (logo - image), token owner is account, caller is account owner" {
    token_account --perm="canTokensAddExtendedInfo" --ext_info_type="image"
    call_ledger --pem=1 --port=8000 token add-ext-info "${SYMBOL}" logo image "png" "\"hello\""
    call_ledger --port=8000 token info "${SYMBOL}"
    assert_output --partial "png"
    assert_output --regexp "binary: \[.*104,.*101,.*108,.*108,.*111,.*\]"
}

@test "$SUITE: can add extended info (memo), token owner is account, caller has add extended info permissions" {
    token_account --perm="canTokensAddExtendedInfo" --ext_info_type="memo"
    call_ledger --pem=2 --port=8000 token add-ext-info "${SYMBOL}" memo "\"My memo\""
    call_ledger --port=8000 token info "${SYMBOL}"
    assert_output --partial "My memo"
}

@test "$SUITE: can add extended info (logo - unicode), token owner is account, caller has add extended info permissions" {
    token_account --perm="canTokensAddExtendedInfo" --ext_info_type="unicode"
    call_ledger --pem=2 --port=8000 token add-ext-info "${SYMBOL}" logo unicode "'∑'"
    call_ledger --port=8000 token info "${SYMBOL}"
    assert_output --partial "'∑'"
}

@test "$SUITE: can add extended info (logo - image), token owner is account, caller has add extended info permission" {
    token_account --perm="canTokensAddExtendedInfo" --ext_info_type="image"
    call_ledger --pem=2 --port=8000 token add-ext-info "${SYMBOL}" logo image "png" "\"hello\""
    call_ledger --port=8000 token info "${SYMBOL}"
    assert_output --partial "png"
    assert_output --regexp "binary: \[.*104,.*101,.*108,.*108,.*111,.*\]"
}

@test "$SUITE: can't add extended info (memo), token owner is account, caller doesn't have add extended info permissions" {
    token_account --perm="canTokensAddExtendedInfo" --ext_info_type="memo"
    call_ledger --pem=3 --port=8000 token add-ext-info "${SYMBOL}" memo "\"My memo\""
    assert_output --partial "Sender needs role 'canTokensAddExtendedInfo' to perform this operation."
}

@test "$SUITE: can't add extended info (logo - unicode), token owner is account, caller doesn't have add extended info permissions" {
    token_account --perm="canTokensAddExtendedInfo" --ext_info_type="unicode"
    call_ledger --pem=3 --port=8000 token add-ext-info "${SYMBOL}" logo unicode "'∑'"
    assert_output --partial "Sender needs role 'canTokensAddExtendedInfo' to perform this operation."
}

@test "$SUITE: can't add extended info (logo - image), token owner is account, caller doesn't have add extended info permission" {
    token_account --perm="canTokensAddExtendedInfo" --ext_info_type="image"
    call_ledger --pem=3 --port=8000 token add-ext-info "${SYMBOL}" logo image "png" "\"hello\""
    assert_output --partial "Sender needs role 'canTokensAddExtendedInfo' to perform this operation."
}

@test "$SUITE: can remove extended info (memo), token owner is account, caller is account owner" {
    token_account --perm="canTokensRemoveExtendedInfo" --ext_info_type="memo"
    call_ledger --pem=1 --port=8000 token remove-ext-info "${SYMBOL}" 0
    call_ledger --port=8000 token info "${SYMBOL}"
    refute_output --partial "My memo"
}

@test "$SUITE: can remove extended info (logo - unicode), token owner is account, caller is account owner" {
    token_account --perm="canTokensRemoveExtendedInfo" --ext_info_type="unicode"
    call_ledger --pem=1 --port=8000 token remove-ext-info "${SYMBOL}" 1
    call_ledger --port=8000 token info "${SYMBOL}"
    refute_output --partial "'∑'"
}

@test "$SUITE: can remove extended info (logo - image), token owner is account, caller is account owner" {
    token_account --perm="canTokensRemoveExtendedInfo" --ext_info_type="image"
    call_ledger --pem=1 --port=8000 token remove-ext-info "${SYMBOL}" 1
    call_ledger --port=8000 token info "${SYMBOL}"
    refute_output --partial "png"
    refute_output --regexp "binary: \[.*104,.*101,.*108,.*108,.*111,.*\]"
}

@test "$SUITE: can remove extended info (memo), token owner is account, caller has remove extended into permission" {
    token_account --perm="canTokensRemoveExtendedInfo" --ext_info_type="memo"
    call_ledger --pem=2 --port=8000 token remove-ext-info "${SYMBOL}" 0
    call_ledger --port=8000 token info "${SYMBOL}"
    refute_output --partial "My memo"
}

@test "$SUITE: can remove extended info (logo - unicode), token owner is account, caller has remove extended info permission" {
    token_account --perm="canTokensRemoveExtendedInfo" --ext_info_type="unicode"
    call_ledger --pem=2 --port=8000 token remove-ext-info "${SYMBOL}" 1
    call_ledger --port=8000 token info "${SYMBOL}"
    refute_output --partial "'∑'"
}

@test "$SUITE: can remove extended info (logo - image), token owner is account, caller has remove extended info permission" {
    token_account --perm="canTokensRemoveExtendedInfo" --ext_info_type="image"
    call_ledger --pem=2 --port=8000 token remove-ext-info "${SYMBOL}" 1
    call_ledger --port=8000 token info "${SYMBOL}"
    refute_output --partial "png"
    refute_output --regexp "binary: \[.*104,.*101,.*108,.*108,.*111,.*\]"
}

@test "$SUITE: can't remove extended info (memo), token owner is account, caller doesn't have remove extended into permission" {
    token_account --perm="canTokensRemoveExtendedInfo" --ext_info_type="memo"
    call_ledger --pem=3 --port=8000 token remove-ext-info "${SYMBOL}" 0
    assert_output --partial "Sender needs role 'canTokensRemoveExtendedInfo' to perform this operation."
}

@test "$SUITE: can't remove extended info (logo - unicode), token owner is account, caller doesn't have remove extended info permission" {
    token_account --perm="canTokensRemoveExtendedInfo" --ext_info_type="unicode"
    call_ledger --pem=3 --port=8000 token remove-ext-info "${SYMBOL}" 1
    assert_output --partial "Sender needs role 'canTokensRemoveExtendedInfo' to perform this operation."
}

@test "$SUITE: can't remove extended info (logo - image), token owner is account, caller doesn't have remove extended info permission" {
    token_account --perm="canTokensRemoveExtendedInfo" --ext_info_type="image"
    call_ledger --pem=3 --port=8000 token remove-ext-info "${SYMBOL}" 1
    assert_output --partial "Sender needs role 'canTokensRemoveExtendedInfo' to perform this operation."
}

@test "$SUITE: MFX metadata" {
    call_ledger --port=8000 token info "${MFX_ADDRESS}"
    assert_output --partial "name: \"Manifest Network Token\""
    assert_output --partial "ticker: \"MFX\""
    assert_output --partial "decimals: 9"
}
