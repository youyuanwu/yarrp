# YARRP
Yet another Rust Reverse Proxy

WIP

## Build
On windows:
```ps1
# configure cmake to download dependencies
cmake -S . -B .\build
# Set symcrypt var.
$env:SYMCRYPT_LIB_PATH="${PWD}\build\_deps\symcrypt_release-src\dll"
# build rust
cargo build --all
# run tests
cargo test
```