fn main() {
    slint_build::compile("ui/app-window.slint").expect("Slint build failed");

    // Embed ui/icon/logo.ico as the exe icon so Windows Explorer shows it.
    // embed_resource is a no-op on non-Windows targets.
    println!("cargo:rerun-if-changed=app.rc");
    println!("cargo:rerun-if-changed=ui/icon/logo.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("app.rc", embed_resource::NONE);
    }
}
