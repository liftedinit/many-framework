GIT_ROOT="$BATS_TEST_DIRNAME/../../"
MFX_ADDRESS=mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz

load '../test_helper/load'

function setup() {
    mkdir "$BATS_TEST_ROOTDIR"

    (
      cd "$GIT_ROOT/docker/e2e/" || exit
      make clean
      for i in {0..2}
      do
          make start-single-node-background ID_WITH_BALANCES="$(identity 1):1000000:$MFX_ADDRESS" NODE="${i}" || {
            echo Could not start nodes... >&3
            exit 1
          }
      done
    ) > /dev/null
    (
      cd "$GIT_ROOT" || exit 1
      cargo build --all-features
    )

    # Give time to the servers to start.
    sleep 30
    timeout 60s bash <<EOT
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

@test "$SUITE: Node can catch up" {
    # Check consistency with nodes [0, 2] up
    check_consistency "$(pem 1)" 1000000 0 1 2
    ledger "$(pem 1)" 0 send "$(identity 2)" 1000 MFX
    sleep 4  # One consensus round.
    check_consistency "$(pem 1)" 999000 0 1 2
    check_consistency "$(pem 2)" 1000 0 1 2

    ledger "$(pem 1)" 1 send "$(identity 2)" 1000 MFX
    ledger "$(pem 1)" 1 send "$(identity 2)" 1000 MFX
    sleep 4  # One consensus round.
    check_consistency "$(pem 1)" 997000 0 1 2
    check_consistency "$(pem 2)" 3000 0 1 2

    ledger "$(pem 1)" 2 send "$(identity 2)" 1000 MFX
    ledger "$(pem 1)" 2 send "$(identity 2)" 1000 MFX
    ledger "$(pem 1)" 2 send "$(identity 2)" 1000 MFX
    sleep 4  # One consensus round.
    check_consistency "$(pem 1)" 994000 0 1 2
    check_consistency "$(pem 2)" 6000 0 1 2

    ledger "$(pem 1)" 0 send "$(identity 2)" 1000 MFX
    ledger "$(pem 1)" 0 send "$(identity 2)" 1000 MFX
    ledger "$(pem 1)" 0 send "$(identity 2)" 1000 MFX
    ledger "$(pem 1)" 0 send "$(identity 2)" 1000 MFX
    sleep 4  # One consensus round.
    check_consistency "$(pem 1)" 990000 0 1 2
    check_consistency "$(pem 2)" 10000 0 1 2

    cd "$GIT_ROOT/docker/e2e/" || exit 1

    # At this point, start the 4th node and check it can catch up
    make start-single-node-background ID_WITH_BALANCES="$(identity 1):1000000" NODE="3" || {
      echo Could not start nodes... >&3
      exit 1
    }

    # Give the 4th node some time to boot
    sleep 30
    timeout 30s bash <<EOT
    while ! many message --server http://localhost:8003 status; do
      sleep 1
    done >/dev/null
EOT
    sleep 12  # Three consensus round.
    check_consistency "$(pem 1)" 990000 0 1 2 3
    check_consistency "$(pem 2)" 10000 0 1 2 3
}

@test "$SUITE: Node can catch up messages older than MANY timeout" {
    # Check consistency with nodes [0, 2] up
    check_consistency "$(pem 1)" 1000000 0 1 2
    ledger "$(pem 1)" 0 send "$(identity 2)" 1000 MFX
    sleep 4  # One consensus round.
    check_consistency "$(pem 1)" 999000 0 1 2
    check_consistency "$(pem 2)" 1000 0 1 2

    # Send a message that's 5 seconds off of being time out.
    many message --timestamp $(($(date +%s) - (4 * 60 + 50))) --server http://localhost:8001 --pem "$(pem 1)" ledger.send '{
        0: "'"$(identity 1)"'",
        1: "'"$(identity 2)"'",
        2: 1000,
        3: "'"$MFX_ADDRESS"'",
    }'
    sleep 4  # One consensus round.
    check_consistency "$(pem 1)" 998000 0 1 2
    check_consistency "$(pem 2)" 2000 0 1 2

    ledger "$(pem 1)" 1 send "$(identity 2)" 1000 MFX
    ledger "$(pem 1)" 1 send "$(identity 2)" 1000 MFX
    sleep 4  # One consensus round.
    check_consistency "$(pem 1)" 996000 0 1 2
    check_consistency "$(pem 2)" 4000 0 1 2

    cd "$GIT_ROOT/docker/e2e/" || exit 1

    # Wait long enough to invalidate the first manual transaction.
    sleep 3

    # At this point, start the 4th node and check it can catch up
    make start-single-node-background ID_WITH_BALANCES="$(identity 1):1000000:$MFX_ADDRESS" NODE="3" || {
      echo Could not start nodes... >&3
      exit 1
    }

    # Give the 4th node some time to boot
    sleep 30
    timeout 60s bash <<EOT
    while ! many message --server http://localhost:8003 status; do
      sleep 1
    done >/dev/null
EOT

    sleep 10  # Three consensus round.
    check_consistency "$(pem 1)" 996000 0 1 2 3
    check_consistency "$(pem 2)" 4000 0 1 2 3
}
