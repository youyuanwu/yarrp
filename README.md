# YARRP
![build](https://github.com/youyuanwu/yarrp/actions/workflows/build.yaml/badge.svg)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://raw.githubusercontent.com/youyuanwu/yarrp/main/LICENSE)

Yet Another Rust Reverse Proxy.

YARRP is a reverse proxy built on top of [hyper](https://github.com/hyperium/hyper) and
[tonic](https://github.com/hyperium/tonic), with pluggable TLS backends:

* [openssl](https://github.com/sfackler/rust-openssl) via the `yarrp-openssl` crate
* [rustls](https://github.com/rustls/rustls) (with Windows CNG / SymCrypt support) via the `yarrp-rustls` crate

> WIP: This project is experimental and primarily targets Windows.

## Dependency
```ps1
# powershell7
winget install Microsoft.PowerShell

# vcpkg. Need to set VCPKG_ROOT var. See https://github.com/microsoft/vcpkg
# openssl
vcpkg install openssl:x64-windows-static-md
```

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

See [DEVELOPMENT.md](./DEVELOPMENT.md) for more details.

## License
This project is licensed under the MIT license.
