#!/bin/sh
rustc -L ./target/debug/ -L dependency=./target/debug/deps/ --extern alloy_sol_macro=./target/debug/deps/liballoy_sol_macro-a58091a95dbdff94.so --extern alloy_json_abi=./target/debug/deps/liballoy_json_abi-eee0a87b1916f8b5.rlib --extern alloy_primitives=./target/debug/deps/liballoy_primitives-f980c0218e29ce68.rlib --extern alloy_sol_types=./target/debug/deps/liballoy_sol_types-4c87198cb969083d.rlib --extern alloy_sol_type_parser=./target/debug/deps/liballoy_sol_type_parser-4e706d9243f13644.rlib --extern ruint=./target/debug/deps/libruint-3dada850bd72bb20.rlib --extern arbitrary=./target/debug/deps/libarbitrary-5631f52023221966.rlib --extern derive_arbitrary=./target/debug/deps/libderive_arbitrary-d27f99037ac777d1.so --extern primitive_types=./target/debug/deps/libprimitive_types-154ebff147e3f8ae.rlib --crate-type dylib -C inline-threshold=25 ./dylib-bindings/src/test_sol.rs
