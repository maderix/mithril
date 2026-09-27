fn main() {
    // Host side links the CUDA driver API (libcuda.so, installed with the
    // driver). GPU device code is compiled separately via docker nvcc.
    println!("cargo:rustc-link-lib=cuda");
}
