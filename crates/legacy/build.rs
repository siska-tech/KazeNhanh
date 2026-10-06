fn main() {
    #[cfg(target_env = "msvc")]
    {
        println!("cargo:rustc-link-lib=advapi32");
        println!("cargo:rustc-link-lib=userenv");
        println!("cargo:rustc-link-lib=crypt32");
        println!("cargo:rustc-link-lib=ole32");
        println!("cargo:rustc-link-lib=secur32");
        println!("cargo:rustc-link-lib=bcrypt");
    }
}
