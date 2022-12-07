GIT_ROOT="$BATS_TEST_DIRNAME/../../../"
MFX_ADDRESS=mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz

load '../../test_helper/load'
load '../../test_helper/ledger'

function setup() {
    mkdir "$BATS_TEST_ROOTDIR"

    skip_if_missing_background_utilities

    if ! [ $CI ]; then
        (
          cd "$GIT_ROOT"
          cargo build --all-features
        )
    fi

    start_ledger --pem "$(pem 0)"
}

function teardown() {
    stop_background_run
}

# Creates a token
# The global variable `SYMBOL` will be set to the new token symbol
function create_token() {
    local ext_info_type
    local pem_arg
    local port
    local error

    while (( $# > 0 )); do
       case "$1" in
         --pem=*) pem_arg=${1}; shift ;;                                    # Identity to create the token with
         --port=*) port=${1}; shift ;;                                      # Port of the ledger server
         --ext_info_type=*) ext_info_type=${1#--ext_info_type=}; shift ;;   # Extended info to add at token creation
         --error) error=true; shift ;;                                      # If this is set, token creation is expected to fail
         --) shift; break ;;
         *) break ;;
       esac
     done

    if [ "${ext_info_type}" = "image" ]; then
        ext_args='logo image "png" "\"hello\""'
    elif [ "${ext_info_type}" = "unicode" ]; then
        ext_args='logo unicode "'∑'"'
    elif [ "$ext_info_type" = "memo" ]; then
        ext_args='memo "My memo"'
    fi

    call_ledger ${pem_arg} ${port} token create "Foobar" "FBR" 9 "$ext_args" "$@"

    if [[ $error ]]; then
        assert_output --partial "Invalid Identity; the sender cannot be anonymous."
    else
        SYMBOL=$(echo $output | grep -oE '"m[a-z0-9]+"' | head -n 1)
        assert [ ${#SYMBOL} -eq 57 ]     # Check the account ID has the right length (55 chars + "")

        assert_output --partial "name: \"Foobar\""
        assert_output --partial "ticker: \"FBR\""
        assert_output --partial "decimals: 9"
        assert_output --regexp "owner:.*$(identity ${pem_arg#--pem=}).*)"

        call_ledger --port=8000 token info "${SYMBOL}"
        if [ "${ext_info_type}" = "image" ]; then
            assert_output --partial "png"
            assert_output --regexp "binary: \[.*104,.*101,.*108,.*108,.*111,.*\]"
        elif [ "${ext_info_type}" = "unicode" ]; then
            assert_output --partial "'∑'"
        elif [ "$ext_info_type" = "memo" ]; then
            assert_output --partial "\"My memo\""
        fi
    fi
}

# Create a new token and assign a new account as the token owner
# `identity(2)` will be assigned the permission given by `--perm`
function token_account() {
    local ext_info_type
    local perm

    while (( $# > 0 )); do
       case "$1" in
         --perm=*) perm=${1#--perm=}; shift ;;                              # Identity to create the token with
         --ext_info_type=*) ext_info_type=${1#--ext_info_type=}; shift ;;   # Extended info to add at token creation
         --) shift; break ;;
         *) break ;;
       esac
     done

    create_token --pem=1 --port=8000 --ext_info_type=${ext_info_type}

    account_id=$(account_create --pem=1 '{ 1: { "'"$(identity 2)"'": ["'${perm}'"] }, 2: [3] }')

    # Account is the new token owner
    call_ledger --pem=1 --port=8000 token update --owner "${account_id}" "${SYMBOL}"
    call_ledger --port=8000 token info "${SYMBOL}"
    assert_output --regexp "owner:.*${account_id}.*)"
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
    create_token --error --port=8000
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
