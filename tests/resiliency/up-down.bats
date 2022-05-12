function timeout() { perl -e 'alarm shift; exec @ARGV' "$@"; }

function setup() {
    load '../test_helper/bats-support/load' # this is required by bats-assert!
    load '../test_helper/bats-assert/load'

    (
      cd ../../docker/e2e/
      make clean
      make start-nodes-background
    ) 2> /dev/null
    (
      cd ../..
      cargo build
    )

    # Give time to the servers to start.
    timeout 30s bash <<EOT
    while ! ../../target/debug/ledger --pem ../id1.pem http://localhost:8000 balance; do
      sleep 1
    done
EOT
}

function teardown() {
    (
      cd ../../docker/e2e/
      make stop-nodes
    ) 2> /dev/null
}

function ledger() {
    local pem="$1"
    local port="$2"
    shift 2
    echo ../../target/debug/ledger --pem "../${pem}" "http://localhost:$((port + 8000))/" "$@" >&2
    run ../../target/debug/ledger --pem "../${pem}" "http://localhost:$((port + 8000))/" "$@"
}

function check_consistency() {
    local pem="$1"
    local expected_balance="$2"
    shift 2

    for port in "$@"; do
        ledger "$pem" "$port" balance
        assert_output --partial " $expected_balance MFX "
    done
}

@test "Network is consistent" {
    # Check consistency with all nodes up.
    check_consistency id1.pem 1000000 0 1 2 3
    ledger id1.pem 0 send "$(many id ../id2.pem)" 1000 MFX
    sleep 4  # One consensus round.
    check_consistency id1.pem 999000 0 1 2 3
    check_consistency id2.pem 1000 0 1 2 3

    ledger id1.pem 1 send "$(many id ../id2.pem)" 2000 MFX
    sleep 4  # One consensus round.
    check_consistency id1.pem 997000 0 1 2 3
    check_consistency id2.pem 3000 0 1 2 3

    ledger id1.pem 2 send "$(many id ../id2.pem)" 3000 MFX
    sleep 4  # One consensus round.
    check_consistency id1.pem 994000 0 1 2 3
    check_consistency id2.pem 6000 0 1 2 3

    ledger id1.pem 3 send "$(many id ../id2.pem)" 4000 MFX
    sleep 4  # One consensus round.
    check_consistency id1.pem 990000 0 1 2 3
    check_consistency id2.pem 10000 0 1 2 3
}

@test "Network is consistent with 1 node down" {
    # Bring down node 3.
    docker stop e2e-tendermint-3-1

    # Check consistency with all nodes up.
    check_consistency id1.pem 1000000 0 1 2
    ledger id1.pem 0 send "$(many id ../id2.pem)" 1000 MFX
    sleep 10  # One consensus round.
    check_consistency id1.pem 999000 0 1 2
    check_consistency id2.pem 1000 0 1 2

    ledger id1.pem 1 send "$(many id ../id2.pem)" 2000 MFX
    sleep 10  # One consensus round.
    check_consistency id1.pem 997000 0 1 2
    check_consistency id2.pem 3000 0 1 2

    ledger id1.pem 2 send "$(many id ../id2.pem)" 3000 MFX
    sleep 10  # One consensus round.
    check_consistency id1.pem 994000 0 1 2
    check_consistency id2.pem 6000 0 1 2

    # Bring it back.
    docker start e2e-tendermint-3-1
    sleep 10
    check_consistency id1.pem 994000 0 1 2 3
    check_consistency id2.pem 6000 0 1 2 3
}
