

run_test () {
	docker-compose run --rm 
}

teardown () {
	docker-compose down
}

run_test
exitcode=$?
teardown

exit $exitcode
