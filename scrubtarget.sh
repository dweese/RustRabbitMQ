# scrubtarget.sh
rustc --version
cargo --version
uname -a
rm -rf target
cargo clean --release
cargo cache --clear
cargo build