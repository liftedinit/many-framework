source "$(dirname "${BASH_SOURCE[0]}")/bats-assert/load.bash"
source "$(dirname "${BASH_SOURCE[0]}")/bats-support/load.bash"

. "$(dirname ${BASH_SOURCE[0]})/bats-utils/helpers"
set_bats_test_suite_name "${BASH_SOURCE[0]%/*}"
remove_bats_test_dirs

source "$(dirname "${BASH_SOURCE[0]}")/bats-utils/background-process"
