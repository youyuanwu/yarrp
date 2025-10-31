fn main() -> Result<(), Box<dyn std::error::Error>> {
    // generate kvstore for grpc
    tonic_prost_build::compile_protos("proto/helloworld.proto")?;

    Ok(())
}
