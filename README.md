# YARRP
Yet another Rust Reverse Proxy

WIP

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