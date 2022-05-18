GIT_ROOT="$BATS_TEST_DIRNAME/../../"

load '../test_helper/load'

function setup() {
    mkdir "$BATS_TEST_ROOTDIR"

    (
      cd "$GIT_ROOT/docker/e2e/" || exit
      make clean
      make start-nodes-background || {
        echo Could not start nodes... >&3
      }
    ) 2> /dev/null
    (
      cd $GIT_ROOT
      cargo build
    )

    # Give time to the servers to start.
    timeout 30s bash <<EOT
    while ! many --pem ../id1.pem http://localhost:8000 status; do
      sleep 1
    done
EOT
}

function teardown() {
    (
      cd "$GIT_ROOT/docker/e2e/"
      make stop-nodes
    ) 2> /dev/null
}

function ledger() {
    local pem="$1"
    local port="$2"
    shift 2
    run "$GIT_ROOT/target/debug/ledger" --pem "$pem" "http://localhost:$((port + 8000))/" "$@"
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

@test "$SUITE: Network is consistent" {
    # Check consistency with all nodes up.
    check_consistency "$(pem 1)" 1000000 0 1 2 3
    ledger "$(pem 1)" 0 send "$(identity 2)" 1000 MFX
    sleep 4  # One consensus round.
    check_consistency "$(pem 1)" 999000 0 1 2 3
    check_consistency "$(pem 2)" 1000 0 1 2 3

    ledger "$(pem 1)" 1 send "$(identity 2)" 2000 MFX
    sleep 4  # One consensus round.
    check_consistency "$(pem 1)" 997000 0 1 2 3
    check_consistency "$(pem 2)" 3000 0 1 2 3

    ledger "$(pem 1)" 2 send "$(identity 2)" 3000 MFX
    sleep 4  # One consensus round.
    check_consistency "$(pem 1)" 994000 0 1 2 3
    check_consistency "$(pem 2)" 6000 0 1 2 3

    ledger "$(pem 1)" 3 send "$(identity 2)" 4000 MFX
    sleep 4  # One consensus round.
    check_consistency "$(pem 1)" 990000 0 1 2 3
    check_consistency "$(pem 2)" 10000 0 1 2 3
}

@test "$SUITE: Network is consistent with 1 node down" {
    # Bring down node 3.
    docker stop e2e-tendermint-3-1

    # Check consistency with all nodes up.
    check_consistency "$(pem 1)" 1000000 0 1 2
    ledger "$(pem 1)" 0 send "$(identity 2)" 1000 MFX
    sleep 10  # One consensus round.
    check_consistency "$(pem 1)" 999000 0 1 2
    check_consistency "$(pem 2)" 1000 0 1 2

    ledger "$(pem 1)" 1 send "$(identity 2)" 2000 MFX
    sleep 10  # One consensus round.
    check_consistency "$(pem 1)" 997000 0 1 2
    check_consistency "$(pem 2)" 3000 0 1 2

    ledger "$(pem 1)" 2 send "$(identity 2)" 3000 MFX
    sleep 10  # One consensus round.
    check_consistency "$(pem 1)" 994000 0 1 2
    check_consistency "$(pem 2)" 6000 0 1 2

    # Bring it back.
    docker start e2e-tendermint-3-1
    sleep 10
    check_consistency "$(pem 1)" 994000 0 1 2 3
    check_consistency "$(pem 2)" 6000 0 1 2 3
}
