GIT_ROOT="$BATS_TEST_DIRNAME/../../../"

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

function create_token() {
    local pem_arg
    local port

    while (( $# > 0 )); do
       case "$1" in
         --pem=*) pem_arg=${1}; shift ;;
         --port=*) port=${1}; shift ;;
         --) shift; break ;;
         *) break ;;
       esac
     done

    call_ledger ${pem_arg} ${port} token create "Foobar" "FBR" 9 "$@"
    assert_output --partial "name: \"Foobar\""
    assert_output --partial "ticker: \"FBR\""
    assert_output --partial "decimals: 9"
    assert_output --regexp "owner:.*$(identity ${pem_arg#--pem=}).*)"
}

function get_symbol() {
    symbol=$(run echo $output | grep -oE '"m[a-z0-9]+"' | head -n 1)
    assert [ ${#symbol} -eq 57 ]     # Check the account ID has the right length (55 chars + "")
    echo ${symbol}
}

@test "$SUITE: can create new token" {
    create_token --pem=1 --port=8000 \
        --initial-distribution "$(identity 1)" 1000 \
        --initial-distribution "$(identity 2)" 1000
    assert_output --regexp "total:.*(.*2000,.*)"
    assert_output --regexp "circulating:.*(.*2000,.*)"
}

@test "$SUITE: can create new token with memo" {
    create_token --pem=1 --port=8000 memo "\"Some memo\""
    call_ledger --port=8000 token info "$(get_symbol)"
    assert_output --partial "Some memo"
}

@test "$SUITE: can create new token with unicode logo" {
    create_token --pem=1 --port=8000 logo unicode "'∑'"
    call_ledger --port=8000 token info "$(get_symbol)"
    assert_output --partial "'∑'"
}

@test "$SUITE: can create new token with image logo" {
    create_token --pem=1 --port=8000 logo image "png" "\"hello\""
    call_ledger --port=8000 token info "$(get_symbol)"
    assert_output --partial "png"
    assert_output --regexp "binary: \[.*104,.*101,.*108,.*108,.*111,.*\]"
}

@test "$SUITE: can't create as anonymous" {
    call_ledger --port=8000 token create "Foobar" "FBR" 9 "$@"
    assert_output --partial "Invalid Identity; the sender cannot be anonymous."
}

@test "$SUITE: can update token" {
    local symbol
    create_token --pem=1 --port=8000
    symbol=$(get_symbol)

    call_ledger --pem=1 --port=8000 token update --name "\"New name\"" \
        --ticker "LLT" \
        --decimals "42" \
        --memo "\"Update memo\"" \
        --owner "$(identity 2)" \
        "${symbol}"

    call_ledger --port=8000 token info "${symbol}"
    assert_output --partial "name: \"New name\""
    assert_output --partial "ticker: \"LLT\""
    assert_output --partial "decimals: 42"
    assert_output --regexp "owner:.*$(identity 2).*)"
}

@test "$SUITE: can't update as non-owner" {
    create_token --pem=1 --port=8000
    call_ledger --pem=2 --port=8000 token update --owner "$(identity 2)" "$(get_symbol)"
    assert_output --partial "Unauthorized to do this operation."
}

@test "$SUITE: can't update as anonymous" {
    create_token --pem=1 --port=8000
    call_ledger --port=8000 token update --owner "$(identity 2)" "$(get_symbol)"
    assert_output --partial "Invalid Identity; the sender cannot be anonymous."
}

@test "$SUITE: can add extended info (memo)" {
    local symbol
    create_token --pem=1 --port=8000
    symbol=$(get_symbol)

    call_ledger --pem=1 --port=8000 token add-ext-info "${symbol}" memo "\"My memo\""

    call_ledger --port=8000 token info "${symbol}"
    assert_output --partial "My memo"
}

@test "$SUITE: can add extended info (logo - unicode)" {
    local symbol
    create_token --pem=1 --port=8000
    symbol=$(get_symbol)

    call_ledger --pem=1 --port=8000 token add-ext-info "${symbol}" logo unicode  "'∑'"

    call_ledger --port=8000 token info "${symbol}"
    assert_output --partial "'∑'"
}

@test "$SUITE: can add extended info (logo - image)" {
    local symbol
    create_token --pem=1 --port=8000
    symbol=$(get_symbol)

    call_ledger --pem=1 --port=8000 token add-ext-info "${symbol}" logo image "png" "\"hello\""

    call_ledger --port=8000 token info "${symbol}"
    assert_output --partial "png"
    assert_output --regexp "binary: \[.*104,.*101,.*108,.*108,.*111,.*\]"
}

@test "$SUITE: can't add extended info as anonymous" {
    local symbol
    create_token --pem=1 --port=8000
    symbol=$(get_symbol)

    # Memo
    call_ledger --port=8000 token add-ext-info "${symbol}" memo "\"My memo\""
    assert_output --partial "Invalid Identity; the sender cannot be anonymous."

    # Logo - unicode
    call_ledger --port=8000 token add-ext-info "${symbol}" logo unicode  "'∑'"
    assert_output --partial "Invalid Identity; the sender cannot be anonymous."

    # Logo - image
    call_ledger --port=8000 token add-ext-info "${symbol}" logo image "png" "\"hello\""
    assert_output --partial "Invalid Identity; the sender cannot be anonymous."
}

@test "$SUITE: can't add extended info as non-owner" {
    local symbol
    create_token --pem=1 --port=8000
    symbol=$(get_symbol)

    # Memo
    call_ledger --pem=2 --port=8000 token add-ext-info "${symbol}" memo "\"My memo\""
    assert_output --partial "Unauthorized to do this operation."

    # Logo - unicode
    call_ledger --pem=2 --port=8000 token add-ext-info "${symbol}" logo unicode  "'∑'"
    assert_output --partial "Unauthorized to do this operation."

    # Logo - image
    call_ledger --pem=2 --port=8000 token add-ext-info "${symbol}" logo image "png" "\"hello\""
    assert_output --partial "Unauthorized to do this operation."
}

@test "$SUITE: can remove extended info (memo)" {
    local symbol
    create_token --pem=1 --port=8000 memo "\"Some memo\""
    symbol=$(get_symbol)

    call_ledger --port=8000 token info "${symbol}"
    assert_output --partial "Some memo"

    # Remove memo
    call_ledger --pem=1 --port=8000 token remove-ext-info "${symbol}" 0
    refute_output --partial "Some memo"
}

@test "$SUITE: can remove extended info (logo)" {
    local symbol
    create_token --pem=1 --port=8000 logo unicode "'∑'"
    symbol=$(get_symbol)

    call_ledger --port=8000 token info "${symbol}"
    assert_output --partial "'∑'"

    # Remove logo
    call_ledger --pem=1 --port=8000 token remove-ext-info "${symbol}" 1
    refute_output --partial "'∑'"
}

@test "$SUITE: can't remove extended info as non-owner" {
    local symbol
    create_token --pem=1 --port=8000
    symbol=$(get_symbol)

    # Memo. We don't care that the token doesn't have one
    call_ledger --pem=2 --port=8000 token remove-ext-info "${symbol}" 0
    assert_output --partial "Unauthorized to do this operation."

    # Logo. We don't care that the token doesn't have one
    call_ledger --pem=2 --port=8000 token remove-ext-info "${symbol}" 1
    assert_output --partial "Unauthorized to do this operation."
}

@test "$SUITE: can't remove extended info as anonymous" {
    local symbol
    create_token --pem=1 --port=8000
    symbol=$(get_symbol)

    # Memo. We don't care that the token doesn't have one
    call_ledger --port=8000 token remove-ext-info "${symbol}" 0
    assert_output --partial "Invalid Identity; the sender cannot be anonymous."
}

@test "$SUITE: can update token, token owner is account, caller is account owner" {
    local symbol
    create_token --pem=1 --port=8000
    symbol=$(get_symbol)

    account_id=$(account_create --pem=1 '{ 1: { "'"$(identity 2)"'": ["canTokensUpdate"] }, 2: [3] }')

    # Account is the new token owner
    call_ledger --pem=1 --port=8000 token update --owner "${account_id}" "${symbol}"
    call_ledger --port=8000 token info "${symbol}"
    assert_output --regexp "owner:.*${account_id}.*)"

    call_ledger --pem=1 --port=8000 token update --name "\"New name\"" "${symbol}"

    call_ledger --port=8000 token info "${symbol}"
    assert_output --partial "name: \"New name\""
}

@test "$SUITE: can update token, token owner is account, caller have update permission" {
    local symbol
    create_token --pem=1 --port=8000
    symbol=$(get_symbol)

    account_id=$(account_create --pem=1 '{ 1: { "'"$(identity 2)"'": ["canTokensUpdate"] }, 2: [3] }')

    # Account is the new token owner
    call_ledger --pem=1 --port=8000 token update --owner "${account_id}" "${symbol}"
    call_ledger --port=8000 token info "${symbol}"
    assert_output --regexp "owner:.*${account_id}.*)"

    call_ledger --pem=2 --port=8000 token update --name "\"New name\"" "${symbol}"

    call_ledger --port=8000 token info "${symbol}"
    assert_output --partial "name: \"New name\""
}

@test "$SUITE: can't update token, token owner is account, caller does't have update permission" {
    local symbol
    create_token --pem=1 --port=8000
    symbol=$(get_symbol)

    account_id=$(account_create --pem=1 '{ 1: { "'"$(identity 2)"'": ["canTokensUpdate"] }, 2: [3] }')

    # Account is the new token owner
    call_ledger --pem=1 --port=8000 token update --owner "${account_id}" "${symbol}"
    call_ledger --port=8000 token info "${symbol}"
    assert_output --regexp "owner:.*${account_id}.*)"

    call_ledger --pem=3 --port=8000 token update --name "\"New name\"" "${symbol}"
    assert_output --partial "Sender needs role 'canTokensUpdate' to perform this operation."
}
