fn main() {
    cxx_build::bridge("core/lib.rs")
        .std("c++17")
        .compile("undertone-bridge");
    println!("cargo:rerun-if-changed=core/lib.rs");
}
