# Build
```ps1
$env:SYMCRYPT_LIB_PATH="${PWD}\build\_deps\symcrypt_release-src\dll"
cargo build
```

## grpc tests
```ps1
.\build\_deps\grpcurl-src\grpcurl.exe `
    -insecure `
    -import-path .\crates\samples\tonic_test\proto -proto helloworld.proto `
    -d '{"name" : "myname" }' `
    127.0.0.1:5047 helloworld.Greeter/SayHello
```