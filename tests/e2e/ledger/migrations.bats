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
        "name": "Account Count Data Attribute",
        "block_height": 20,
        "issue": "https://github.com/liftedinit/many-framework/issues/190"
      },
      {
        "name": "Dummy Hotfix",
        "block_height": 0,
        "disabled": true
      },
      {
        "name": "Block 9400",
        "block_height": 0,
        "disabled": true
      }
    ]' > "$BATS_TEST_ROOTDIR/migrations.json"

    start_ledger --pem "$(pem 0)" \
        "--migrations-config=$BATS_TEST_ROOTDIR/migrations.json"
}

@test "$SUITE: Missing migration (bad length)" {
    echo '
    [
      {
        "name": "Dummy Hotfix",
        "block_height": 0,
        "disabled": true
      },
      {
        "name": "Block 9400",
        "block_height": 0,
        "disabled": true
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
        "name": "Dummy Hotfix",
        "block_height": 20
      },
      {
        "name": "Dummy Hotfix",
        "block_height": 0,
        "disabled": true
      },
      {
        "name": "Block 9400",
        "block_height": 0,
        "disabled": true
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
        "name": "Foobar",
        "block_height": 20
      },
      {
        "name": "Dummy Hotfix",
        "block_height": 0,
        "disabled": true
      },
      {
        "name": "Block 9400",
        "block_height": 0,
        "disabled": true
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
        "name": "Account Count Data Attribute",
        "block_height": 20,
        "issue": "https://github.com/liftedinit/many-framework/issues/190"
      },
      {
        "name": "Dummy Hotfix",
        "block_height": 0,
        "disabled": true
      },
      {
        "name": "Block 9400",
        "block_height": 40,
        "disabled": true
      }
    ]' > "$BATS_TEST_ROOTDIR/migrations.json"

    start_ledger --background_output="block_height: 40, disabled: true" \
        --pem "$(pem 0)" \
        "--migrations-config=$BATS_TEST_ROOTDIR/migrations.json"
}
