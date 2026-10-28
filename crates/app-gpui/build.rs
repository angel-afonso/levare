//! Embeds Levare's icon in the Windows executable. Other targets need nothing.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // A build script's `cfg` describes the machine running it, so this part
    // only exists on a Windows host, where the resource compiler is. The target
    // is checked too, so a Windows host building for another system skips it.
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=levare.rc");
        println!("cargo:rerun-if-changed=../../assets/branding/levare/levare.ico");
        embed_resource::compile("levare.rc", embed_resource::NONE)
            .manifest_required()
            .expect("embed Levare's icon in levare.exe");
    }
}
