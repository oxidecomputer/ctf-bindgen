#!/bin/bash

pushd c
./build.sh
popd

cargo run c/libfoo.so

LD_LIBRARY_PATH=`pwd`/c cargo test
