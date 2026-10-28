//! The same icon is embedded for `cargo run` and copied into the macOS bundle.

use gpui::{Image, ImageFormat};
use std::sync::{Arc, OnceLock};

/// Reuse the approved app icon in the titlebar, without filesystem lookups or
/// allocating and hashing its encoded bytes on each editor render.
pub fn image() -> Arc<Image> {
    static IMAGE: OnceLock<Arc<Image>> = OnceLock::new();
    IMAGE
        .get_or_init(|| {
            Arc::new(Image::from_bytes(
                ImageFormat::Png,
                include_bytes!("../../../assets/branding/levare/icon-macos-v2.png").to_vec(),
            ))
        })
        .clone()
}

#[cfg(target_os = "macos")]
#[allow(deprecated, unexpected_cfgs)]
pub fn install() {
    use cocoa::{
        appkit::{NSApp, NSApplication, NSImage},
        base::nil,
        foundation::{NSAutoreleasePool, NSData},
    };
    use objc::{msg_send, sel, sel_impl};

    let bytes = include_bytes!("../../../assets/branding/levare/Levare.icns");
    // SAFETY: called from GPUI's startup callback on the AppKit main thread.
    // NSData copies the embedded bytes; NSApplication retains the image before
    // our owned reference is released and the autorelease pool is drained.
    unsafe {
        let pool = NSAutoreleasePool::new(nil);
        let data = NSData::dataWithBytes_length_(nil, bytes.as_ptr().cast(), bytes.len() as u64);
        let image = NSImage::initWithData_(NSImage::alloc(nil), data);
        if image != nil {
            NSApp().setApplicationIconImage_(image);
            let _: () = msg_send![image, release];
        } else {
            crate::diagnostics::log("No se pudo cargar el icono de Levare");
        }
        let _: () = msg_send![pool, drain];
    }
}

/// Nothing to install at run time elsewhere. On Windows the icon is resource 1
/// of levare.exe (`build.rs`), which GPUI gives every window. On Linux the
/// desktop takes it from the desktop entry that matches the window's app id,
/// `dev.partitura.editor`; GPUI has no API for a window icon.
#[cfg(not(target_os = "macos"))]
pub fn install() {}
