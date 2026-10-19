fn main() {
    println!("cargo:rerun-if-changed=assets/mem-map.rc");
    println!("cargo:rerun-if-changed=assets/mem-map.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile_for("assets/mem-map.rc", ["mem-map"], embed_resource::NONE)
            .manifest_required()
            .expect("compile the Mem Map icon resource");
    }
}
