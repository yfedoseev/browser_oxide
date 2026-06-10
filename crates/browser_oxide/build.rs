fn main() {
    // Link OSMesa for software OpenGL rendering (WebGL support)
    if cfg!(feature = "webgl-render") && cfg!(unix) {
        println!("cargo:rustc-link-lib=OSMesa");
    }
}
