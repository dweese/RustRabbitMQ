#!/bin/bash
./scrubtarget.sh
cargo update
cargo build
git status
git add .
git commit -m "Remove unused imports and fix cargo run"
git push
source .env && cargo run > cargo.log 2>&1