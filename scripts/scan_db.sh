#! /usr/bin/zsh

# TODO: Refactor with parallel
# TODO: Add progress bar
function scan() {
    while read -r line; do
        key=${line%%"==>"*}
        value=${line%%*"==>"}
        key_value=$(echo ${key} | xxd -r)
        value_len=${#value}

        echo Key hex: $key >> ${1%.txt}_ascii.txt
        echo Key: $key_value >> ${1%.txt}_ascii.txt
        # TODO: Make this better.
        for x in {1..$value_len};
        do
            if echo "$value" | colrm 1 $x | cbor-diag 2>&- >> ${1%.txt}_ascii.txt;
            then
                break;
            fi
            if echo "$value" | colrm 1 $x | xargs many -q -q -q id 2>&- >> ${1%.txt}_ascii.txt;
            then
                break;
            fi
        done
    done <$1
}

# Check dependencies
for bin in rocksdb-ldb many cbor-diag; do
  which $bin > /dev/null || {
    echo "You need the $bin binary"
    echo ""
    exit 1
  }
done

tmp_dir=$(mktemp -d)
tmp_lhs=$(mktemp -p ${tmp_dir} --suffix=_lhs.txt)
tmp_rhs=$(mktemp -p ${tmp_dir} --suffix=_rhs.txt)

echo Dumping $1 to ${tmp_lhs}
rocksdb-ldb --db=$1 --hex dump > ${tmp_lhs}

echo Dumping $2 to ${tmp_rhs}
rocksdb-ldb --db=$2 --hex dump > ${tmp_rhs}

echo Extracting diff lines from ${tmp_lhs} in ${tmp_lhs%.txt}_diff.txt
diff ${tmp_lhs} ${tmp_rhs} | grep "^<" | colrm 1 2 | head -n -1 > ${tmp_lhs%.txt}_diff.txt

echo Extracting diff lines from ${tmp_rhs} in ${tmp_rhs%.txt}_diff.txt
diff ${tmp_lhs} ${tmp_rhs} | grep "^>" | colrm 1 2 | head -n -1 > ${tmp_rhs%.txt}_diff.txt

echo Converting hex to ascii ${tmp_lhs%.txt}_diff_ascii.txt
scan ${tmp_lhs%.txt}_diff.txt

echo Converting hex to ascii ${tmp_rhs%.txt}_diff_ascii.txt
scan ${tmp_rhs%.txt}_diff.txt
