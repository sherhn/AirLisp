//! Embeds the application icon and version info into the Windows executable.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=air.rc");
        println!("cargo:rerun-if-changed=../../assets/icon/air.ico");
        let _ = embed_resource::compile("air.rc", embed_resource::NONE);
    }
}
