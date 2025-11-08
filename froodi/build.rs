fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-check-cfg=cfg(const_type_id)");
    if let Some(true) = version_check::is_min_version("1.91.0") {
        println!("cargo:rustc-cfg=const_type_id");
    }
}
