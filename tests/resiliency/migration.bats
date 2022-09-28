GIT_ROOT="$BATS_TEST_DIRNAME/../../"

load '../test_helper/load'

function setup() {
    mkdir "$BATS_TEST_ROOTDIR"

    (
      cd "$GIT_ROOT/docker/e2e/" || exit
      make clean
      make $(ciopt start-nodes-dettached) ABCI_TAG=$(img_tag) LEDGER_TAG=$(img_tag) ID_WITH_BALANCES="$(identity 1):1000000" MIGRATIONS=$GIT_ROOT/tests/resiliency/migrations.toml || {
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

@test "$SUITE: migrations work" {
    check_consistency --pem=1 --balance=1000000 --id="$(identity 1)" 8000 8001 8002 8003
    many message --server="http://localhost:8000" data.info
    assert_output --partial "[[]]"
}
