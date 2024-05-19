use std::io::Result;
fn main() -> Result<()> {
    prost_build::compile_protos(
        &[
            "rpc.proto",
            "swarm.proto",
            "person.proto",
            "primitive.proto",
            "signaling.proto",
        ],
        &["pb/"],
    )?;
    Ok(())
}
