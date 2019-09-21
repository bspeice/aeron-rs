use cmake::Config;
use dunce::canonicalize;
use std::env;
use std::path::{Path, PathBuf};

pub enum LinkType {
    Dynamic,
    Static,
}

impl LinkType {
    fn detect() -> LinkType {
        if cfg!(feature = "static") {
            LinkType::Static
        } else {
            LinkType::Dynamic
        }
    }

    fn link_lib(&self) -> &'static str {
        match self {
            LinkType::Dynamic => "dylib=",
            LinkType::Static => "static=",
        }
    }

    fn target_name(&self) -> &'static str {
        match self {
            LinkType::Dynamic => "aeron_driver",
            LinkType::Static => "aeron_driver_static",
        }
    }
}

pub fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=bindings.h");

    let aeron_path = canonicalize(Path::new("./aeron")).unwrap();
    let header_path = aeron_path.join("aeron-driver/src/main/c");
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    let link_type = LinkType::detect();
    println!(
        "cargo:rustc-link-lib={}{}",
        link_type.link_lib(),
        link_type.target_name()
    );
    let cmake_output = Config::new(&aeron_path)
        .build_target(link_type.target_name())
        .build();

    // Trying to figure out the final path is a bit weird;
    // For Linux/OSX, it's just build/lib
    // For Windows, it's build/lib/{profile}
    let base_lib_dir = cmake_output.join("build").join("lib");
    println!("cargo:rustc-link-search=native={}", base_lib_dir.display());
    // Because the `cmake_output` path is different for debug/release, we're not worried
    // about accidentally linking in the wrong library
    println!(
        "cargo:rustc-link-search=native={}",
        base_lib_dir.join("Debug").display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        base_lib_dir.join("Release").display()
    );

    println!("cargo:include={}", header_path.display());
    let bindings = bindgen::Builder::default()
        .clang_arg(&format!("-I{}", header_path.display()))
        .header("bindings.h")
        .generate()
        .expect("Unable to generate aeron_driver bindings");

    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
