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
    let lib_dir = lib_output_dir(&cmake_output);
    println!(
        "cargo:rustc-link-search=native={}",
        lib_dir.display()
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

fn lib_output_dir(cmake_dir: &PathBuf) -> PathBuf {
    if cfg!(target_os = "windows") {
        if cmake_dir.join("build/lib/Debug").exists() {
            cmake_dir.join("build/lib/Debug")
        } else {
            cmake_dir.join("build/lib/Release")
        }
    } else {
        cmake_dir.join("build/lib")
    }
}