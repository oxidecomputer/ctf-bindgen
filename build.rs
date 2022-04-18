fn main() {
    // for linking with test libraries
    println!("cargo:rustc-link-search=tests/c");
}
