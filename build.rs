fn main() {
    println!("cargo:rerun-if-changed=src/lib.rs");

    let _build = cxx_build::bridge("src/lib.rs");
}
