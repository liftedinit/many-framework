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

}

function teardown() {
    stop_background_run
}

@test "$SUITE: Load migrations" {
    echo '
    [
      {
        "type": "Account Count Data Attribute",
        "block_height": 20,
        "issue": "https://github.com/liftedinit/many-framework/issues/190"
      },
      {
        "type": "Dummy Hotfix",
        "block_height": 0,
        "status": "Disabled"
      },
      {
        "type": "Block 9400",
        "block_height": 0,
        "status": "Disabled"
      }
    ]' > "$BATS_TEST_ROOTDIR/migrations.json"

    start_ledger --pem "$(pem 0)" \
        "--migrations-config=$BATS_TEST_ROOTDIR/migrations.json"
}

@test "$SUITE: Missing migration (bad length)" {
    echo '
    [
      {
        "type": "Dummy Hotfix",
        "block_height": 0,
        "status": "Disabled"
      },
      {
        "type": "Block 9400",
        "block_height": 0,
        "status": "Disabled"
      }
    ]' > "$BATS_TEST_ROOTDIR/migrations.json"

    start_ledger --background_output="Migration configuration file is missing migration\(s\)"\
        --pem "$(pem 0)" \
        "--migrations-config=$BATS_TEST_ROOTDIR/migrations.json"
}

@test "$SUITE: Missing migration (right length, duplicate)" {
    echo '
    [
      {
        "type": "Dummy Hotfix",
        "block_height": 20
      },
      {
        "type": "Dummy Hotfix",
        "block_height": 0,
        "status": "Disabled"
      },
      {
        "type": "Block 9400",
        "block_height": 0,
        "status": "Disabled"
      }
    ]' > "$BATS_TEST_ROOTDIR/migrations.json"

    start_ledger --background_output="Migration configuration file is missing" \
        --pem "$(pem 0)" \
        "--migrations-config=$BATS_TEST_ROOTDIR/migrations.json"
}

@test "$SUITE: Unsupported migration type" {
    echo '
    [
      {
        "type": "Foobar",
        "block_height": 20
      },
      {
        "type": "Dummy Hotfix",
        "block_height": 0,
        "status": "Disabled"
      },
      {
        "type": "Block 9400",
        "block_height": 0,
        "status": "Disabled"
      }
    ]' > "$BATS_TEST_ROOTDIR/migrations.json"

    start_ledger --background_output="Unsupported migration type" \
        --pem "$(pem 0)" \
        "--migrations-config=$BATS_TEST_ROOTDIR/migrations.json"
}

@test "$SUITE: Can disable" {
    echo '
    [
      {
        "type": "Account Count Data Attribute",
        "block_height": 20,
        "issue": "https://github.com/liftedinit/many-framework/issues/190"
      },
      {
        "type": "Dummy Hotfix",
        "block_height": 0,
        "status": "Disabled"
      },
      {
        "type": "Block 9400",
        "block_height": 40,
        "status": "Disabled"
      }
    ]' > "$BATS_TEST_ROOTDIR/migrations.json"

    start_ledger --background_output="block_height: 40.*status: Disabled" \
        --pem "$(pem 0)" \
        "--migrations-config=$BATS_TEST_ROOTDIR/migrations.json"
}
