# shellcheck disable=SC1128
#!/bin/bash
./scrubtarget.sh
cargo update
cargo build
git status
git add .
git commit -m "Fix Cargo.toml and src/rabbitmq.rs"
git push
env $(cat .env | xargs) cargo run > cargo.log 2>&1