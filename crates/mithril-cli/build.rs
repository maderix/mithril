// `cuda`: the GPU backend is built in (the `gpu` feature, on by default, on
// Linux, where the NVIDIA driver provides libcuda). Elsewhere, or with
// --no-default-features, the CLI is CPU only.
fn main() {
    println!("cargo:rustc-check-cfg=cfg(cuda)");
    let linux = std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "linux");
    if linux && std::env::var_os("CARGO_FEATURE_GPU").is_some() {
        println!("cargo:rustc-cfg=cuda");
    }
}
