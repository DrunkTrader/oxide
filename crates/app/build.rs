//! Embeds the Oxide icon into `oxide-app.exe` so Explorer, the taskbar and
//! installer shortcuts show it. A no-op for every non-Windows target.

fn main() {
    println!("cargo:rerun-if-changed=../../assets/oxide.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("../../assets/oxide.ico");
        if let Err(error) = resource.compile() {
            panic!("Cannot embed the Windows application icon: {error}");
        }
    }
}
