fn main() {
    println!("cargo:rustc-link-lib=dylib=dinput8");
    println!("cargo:rustc-link-lib=dylib=dxguid");
}
