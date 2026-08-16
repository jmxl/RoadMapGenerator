fn main() {
    slint_build::compile("ui/app-window.slint").expect("Slint build failed");

    // Embed ui/icon/logo.ico as the exe icon so Windows Explorer shows it.
    // winresource is a no-op on non-Windows targets.
    println!("cargo:rerun-if-changed=ui/icon/logo.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("ui/icon/logo.ico");
        res.compile().expect("winresource compile failed");
    }
}