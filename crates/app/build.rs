fn main() {
    let cfg = slint_build::CompilerConfiguration::new().with_style("fluent-dark".into());
    slint_build::compile_with_config("ui/main.slint", cfg).unwrap();
    // Embed the app icon on Windows only.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=app.rc");
        println!("cargo:rerun-if-changed=../../assets/eposcontrol.ico");
        let _ = embed_resource::compile("app.rc", embed_resource::NONE);
    }
}
