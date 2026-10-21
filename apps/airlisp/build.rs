//! Embeds the application icon and version info into the Windows executable.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=airlisp.rc");
        println!("cargo:rerun-if-changed=../../assets/icon/airlisp.ico");
        let _ = embed_resource::compile("airlisp.rc", embed_resource::NONE);
    }
}
