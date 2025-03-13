# scrubtarget.sh
rustc --version
cargo --version
uname -a
git clean -fdx
rm -rf target
cargo clean --release
cargo build