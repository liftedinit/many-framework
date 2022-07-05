# many-framework

![ci](https://img.shields.io/github/workflow/status/liftedinit/many-framework/CI)
![license](https://img.shields.io/github/license/liftedinit/many-framework)

A collection of applications based on the [MANY protocol](https://github.com/many-protocol) and [MANY libraries](https://github.com/liftedinit/many-rs/)

Features
- A ledger client/server
- A key-value store client/server
- An application blockchain interface (ABCI)
- A http proxy
- A 4-nodes end-to-end Docker demo
- CLI developer's tools

# References

- Concise Binary Object Representation (CBOR): [RFC 8949](https://www.rfc-editor.org/rfc/rfc8949.html)
- CBOR Object Signing and Encryption (COSE): [RFC 8152](https://datatracker.ietf.org/doc/html/rfc8152)
- Platform-independent API to cryptographic tokens: [PKCS #11](https://docs.oasis-open.org/pkcs11/pkcs11-base/v2.40/os/pkcs11-base-v2.40-os.html)
- Blockchain application platform: [Tendermint](https://docs.tendermint.com/master/)
- Persistent key-value store: [RocksDB](https://rocksdb.org/)

# Developer tools
- CBOR playground: [CBOR.me](https://cbor.me)
- CBOR diagnostic utilities: [cbor-diag](https://github.com/cabo/cbor-diag)
- Software Hardware Security Module (HSM): [SoftHSM2](https://github.com/opendnssec/SoftHSMv2)
- Bash automated testing system: [bats-core](https://github.com/bats-core/bats-core)
- Container engine: [Docker](https://www.docker.com/)
- The MANY libraries: [many-rs](https://github.com/liftedinit/many-rs)

# Installation

1. Update your package database
```shell
# Ubuntu
$ sudo apt update

# CentOS
$ sudo yum update

# Archlinux
$ sudo pacman -Syu

# macOS
$ brew update
```
1. Install Rust using [rustup](https://rustup.rs/)
```shell
# Ubuntu/CentOS
$ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
$ source $HOME/.cargo/env

# Archlinux
$ sudo pacman -S rustup

# macOS
$ brew install rustup-init
```
2. Install build dependencies
```shell
# Ubuntu
$ sudo apt install build-essential pkg-config clang libssl-dev libsofthsm2

# CentOS
$ sudo yum install clang gcc softhsm git pkgconf

# Archlinux
$ sudo pacman -S clang gcc softhsm git pkgconf
```
3. Build `many-framework`
```shell
$ git clone https://github.com/liftedinit/many-framework.git
$ cd many-framework
$ cargo build
```
4. Run tests
```shell
$ cargo test
```

# Usage example
Below are some examples of how to use the different CLI. 

## Requirements 
1. Install the `many` CLI

```shell 
$ cargo install --git https://github.com/liftedinit/many-rs many-cli
```

2. Generate a new key and get its MANY ID
```shell
# Generate a new Ed25519 key
$ openssl genpkey -algorithm Ed25519 -out id1.pem

# Get the MANY ID of the key
$ many id id1.pem
maeguvtgcrgXXXXXXXXXXXXXXXXXXXXXXXXwqg6ibizbmflicz
```

3. Assign some tokens to your MANY ID by adding it to the `initial` section of the `staging/ledger_state.json5` file
```json5
    "maeguvtgcrgXXXXXXXXXXXXXXXXXXXXXXXXwqg6ibizbmflicz": {
      "MFX": 123456789
    }
```

4. (Dev) Comment the `hash` entry from the `staging/ledger_state.json5` file
```json5
  // hash: "fc0041ca4f7d959fe9e5a337e175bd8a68942cad76745711a3daf820a159f7eb"
```

## Run a Ledger server
```shell
# Follow the instructions from the `Requirements` section above before running this example.

# Run the ledger server using the provided initial state and key. 
# Create a clean persistent storage.
$ ./target/debug/many-ledger --pem id1.pem --state ./staging/ledger_state.json5 --persistent ledger.db --clean
2022-07-05T18:21:45.598272Z  INFO many_ledger: address="maeguvtgcrgXXXXXXXXXXXXXXXXXXXXXXXXwqg6ibizbmflicz"
2022-07-05T18:21:45.625108Z  INFO many_ledger::module: height=0 hash="fc0041ca4f7d959fe9e5a337e175bd8a68942cad76745711a3daf820a159f7eb"
```

## Query balance
```shell
# Follow the instructions from the `Requirements` section above before running this example.

# You will need to have a running ledger server before running this example.
# See section `Run a Ledger server` above.

$ ./target/debug/ledger --pem id1.pem balance
   123456789 MFX (mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz)
```

## Send tokens
```shell
# Follow the instructions from the `Requirements` section above before running this example.

# You will need to have a running ledger server before running this example.
# See section `Run a Ledger server` above.

# Generate a random key and get its MANY ID
$ openssl genpkey -algorithm Ed25519 -out tmp.pem
$ many id tmp.pem
maf4byfbrz7dcc72tgb5zbof75cs52wg2fwbc2fdf467qj2qcx

# Send tokens from id1.pem to tmp.pem
$ ./target/debug/ledger --pem id1.pem send maf4byfbrz7dcc72tgb5zbof75cs52wg2fwbc2fdf467qj2qcx 10000 MFX

# Check the balance of the new ID
$ ./target/debug/ledger --pem tmp.pem balance
       10000 MFX (mqbfbahksdwaqeenayy2gxke32hgb7aq4ao4wt745lsfs6wiaaaaqnz)
```

# Contributing

1. Read our [Contributing Guidelines](https://github.com/liftedinit/.github/blob/main/docs/CONTRIBUTING.md)
2. Fork the project (https://github.com/liftedinit/many-framework/fork)
3. Create a feature branch (`git checkout -b feature/fooBar`)
4. Commit your changes (`git commit -am 'Add some fooBar'`)
5. Push to the branch (`git push origin feature/fooBar`)
6. Create a new Pull Request
