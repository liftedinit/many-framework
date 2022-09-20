GIT_ROOT="$BATS_TEST_DIRNAME/../../"

load '../test_helper/load'

function setup() {
    mkdir "$BATS_TEST_ROOTDIR"

    skip_if_missing_background_utilities

    if ! [ $CI ]; then
        (
          cd "$GIT_ROOT"
          cargo build
        )
    fi

    run_in_background "$GIT_ROOT/target/debug/many-kvstore" \
          -v \
          --clean \
          --persistent "$(mktemp -d)" \
          --state "$GIT_ROOT/staging/kvstore_state.json5" \
          --pem "$(pem 0)"

    wait_for_background_output "Running accept thread"
}

function teardown() {
    stop_background_run
}

@test "$SUITE: can put and get data" {
  call_kvstore --pem=1 --port=8000 put "010203" "foobar"
  call_kvstore --pem=1 --port=8000 get "010203"
  assert_output --partial "foobar"
}

@test "$SUITE: can put and query data" {
  call_kvstore --pem=1 --port=8000 put "010203" "foobar"
  call_kvstore --pem=1 --port=8000 query "010203"
  assert_output --partial "$(identity 1)"
}

@test "$SUITE: can disable data" {
  call_kvstore --pem=1 --port=8000 put "010203" "foobar"
  call_kvstore --pem=1 --port=8000 disable "010203"
  call_kvstore --pem=1 --port=8000 get "010203"
  assert_output --partial "The key was disabled by its owner."
}

@test "$SUITE: can put data on-behalf of" {
  account_id=$(account_create --pem=1 '{ 1: { "'"$(identity 2)"'": ["canKvStorePut"] }, 2: [2] }')
  call_kvstore --pem=2 --port=8000 --alt-owner "$account_id" put "040506" "foobar"
  call_kvstore --pem=1 --port=8000 get "040506"
  assert_output --partial "foobar"
}

@test "$SUITE: can disable data on-behalf of" {
  account_id=$(account_create --pem=1 '{ 1: { "'"$(identity 2)"'": ["canKvStoreDisable"] }, 2: [2] }')
  call_kvstore --pem=1 --port=8000 --alt-owner "$account_id" put "060708" "foobar"
  call_kvstore --pem=2 --port=8000 --alt-owner "$account_id" disable "060708"
  call_kvstore --pem=1 --port=8000 get "060708"
  assert_output --partial "The key was disabled by its owner."
}

@test "$SUITE: unable to disable an empty key" {
  call_kvstore --pem=1 --port=8000 disable "010203"
  assert_output --partial "Unable to disable an empty key."
}

@test "$SUITE: can disable with reason" {
  call_kvstore --pem=1 --port=8000 put "112233" "foobar"
  call_kvstore --pem=1 --port=8000 disable --reason "sad" "112233"
  call_kvstore --pem=1 --port=8000 get "112233"
  assert_output --partial "The key was disabled by its owner."

  call_kvstore --pem=1 --port=8000 query "112233"
  assert_output --partial "sad"
}
