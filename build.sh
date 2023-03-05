#!/bin/sh
npm i
npm run uno
cargo install trunk
rustup target add wasm32-unknown-unknown
trunk build --release
