fn main() {
    let cfg = slint_build::CompilerConfiguration::new().with_style("fluent-dark".into());
    slint_build::compile_with_config("ui/main.slint", cfg).unwrap();
    println!("cargo:rerun-if-changed=app.rc");
    println!("cargo:rerun-if-changed=../../assets/eposcontrol.ico");
    let _ = embed_resource::compile("app.rc", embed_resource::NONE);
}
