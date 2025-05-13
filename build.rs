use std::env;

fn main() {
    if let Ok(path) = env::var("NIXF_TIDY_PATH") {
        println!("cargo:rustc-env=NIXF_TIDY_PATH={}", path);
    } else {
        println!("cargo:rustc-env=NIXF_TIDY_PATH=nixf-tidy");
    }
}
