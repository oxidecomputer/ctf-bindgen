#!/bin/bash
#:
#: name = "build-and-test"
#: variety = "basic"
#: target = "helios-20220404"
#: rust_toolchain = "nightly-2021-09-03"
#: output_rules = [
#:   "/work/debug/*",
#:   "/work/release/*",
#: ]
#:

set -o errexit
set -o pipefail
set -o xtrace

cargo --version
rustc --version

banner "build"
cargo clean
ptime -m cargo build
ptime -m cargo build --release
pushd tests
./prepare.sh
popd

for x in debug release
do
    mkdir -p /work/$x
    cp target/$x/ctf-bindgen /work/$x/ctf-bindgen
done

banner "check"
cargo fmt -- --check
cargo clippy

banner "test"
pushd tests
./run.sh
popd
