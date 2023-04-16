fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("pb/spotify-remote.proto")?;

    // let proto_file = "./pb/spotify-remote.proto";
    // let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // tonic_build::configure()
    //     .build_client(true)
    //     .build_server(true)
    //     .file_descriptor_set_path(out_dir.join("spotify_remote_descriptor.bin"))
    //     .out_dir("./src")
    //     .compile(&[proto_file], &["pb"])?;

    Ok(())
}
