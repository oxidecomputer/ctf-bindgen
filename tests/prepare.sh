#!/bin/bash

pushd c
./build.sh
popd

cargo run c/libfoo.so
