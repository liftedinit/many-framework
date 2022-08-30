GIT_ROOT="$BATS_TEST_DIRNAME/../../"

load '../test_helper/load'

function setup() {
    mkdir "$BATS_TEST_ROOTDIR"

    (
      cd "$GIT_ROOT/docker/e2e/" || exit
      make clean
      make $(ciopt start-nodes-dettached) ABCI_TAG=$(img_tag) LEDGER_TAG=$(img_tag) ID_WITH_BALANCES="$(identity 1):1000000" || {
        echo Could not start nodes... >&3
        exit 1
      }
    ) > /dev/null

    # Give time to the servers to start.
    sleep 30
    timeout 30s bash <<EOT
    while ! many message --server http://localhost:8000 status; do
      sleep 1
    done >/dev/null
EOT
}

function teardown() {
    (
      cd "$GIT_ROOT/docker/e2e/" || exit 1
      make stop-nodes
    ) 2> /dev/null

    # Fix for BATS verbose run/test output gathering
    cd "$GIT_ROOT/tests/resiliency/" || exit 1
}

@test "$SUITE: Network is consistent" {
    # Check consistency with all nodes up.
    check_consistency "$(pem 1)" 1000000 "$(pem 1)" 0 1 2 3
    call_ledger "$(pem 1)" 0 send "$(identity 2)" 1000 MFX
    sleep 4  # One consensus round.
    check_consistency "$(pem 1)" 999000 "$(pem 1)" 0 1 2 3
    check_consistency "$(pem 2)" 1000 "$(pem 2)" 0 1 2 3

    call_ledger "$(pem 1)" 1 send "$(identity 2)" 2000 MFX
    sleep 4  # One consensus round.
    check_consistency "$(pem 1)" 997000 "$(pem 1)" 0 1 2 3
    check_consistency "$(pem 2)" 3000 "$(pem 2)" 0 1 2 3

    call_ledger "$(pem 1)" 2 send "$(identity 2)" 3000 MFX
    sleep 4  # One consensus round.
    check_consistency "$(pem 1)" 994000 "$(pem 1)" 0 1 2 3
    check_consistency "$(pem 2)" 6000 "$(pem 2)" 0 1 2 3

    call_ledger "$(pem 1)" 3 send "$(identity 2)" 4000 MFX
    sleep 4  # One consensus round.
    check_consistency "$(pem 1)" 990000 "$(pem 1)" 0 1 2 3
    check_consistency "$(pem 2)" 10000 "$(pem 2)" 0 1 2 3
}

@test "$SUITE: Network is consistent with 1 node down" {
    cd "$GIT_ROOT/docker/e2e/" || exit 1

    # Bring down node 3.
    make stop-single-node-3

    # Check consistency with all nodes up.
    check_consistency "$(pem 1)" 1000000 "$(pem 1)" 0 1 2
    call_ledger "$(pem 1)" 0 send "$(identity 2)" 1000 MFX
    sleep 10  # One consensus round.
    check_consistency "$(pem 1)" 999000 "$(pem 1)" 0 1 2
    check_consistency "$(pem 2)" 1000 "$(pem 2)" 0 1 2

    call_ledger "$(pem 1)" 1 send "$(identity 2)" 2000 MFX
    sleep 10  # One consensus round.
    check_consistency "$(pem 1)" 997000 "$(pem 1)" 0 1 2
    check_consistency "$(pem 2)" 3000 "$(pem 2)" 0 1 2

    call_ledger "$(pem 1)" 2 send "$(identity 2)" 3000 MFX
    sleep 10  # One consensus round.
    check_consistency "$(pem 1)" 994000 "$(pem 1)" 0 1 2
    check_consistency "$(pem 2)" 6000 "$(pem 2)" 0 1 2

    # Bring it back.
    make $(ciopt start-single-node-dettached)-3 ABCI_TAG=$(img_tag) LEDGER_TAG=$(img_tag) || {
        echo Could not start nodes... >&3
        exit 1
    }

    # Give time to the servers to start.
    timeout 60s bash <<EOT
    while ! many message --server http://localhost:8003 status; do
      sleep 1
    done >/dev/null
EOT
    sleep 10
    check_consistency "$(pem 1)" 994000 "$(pem 1)" 0 1 2 3
    check_consistency "$(pem 2)" 6000 "$(pem 2)" 0 1 2 3
}
