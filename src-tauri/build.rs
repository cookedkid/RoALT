fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rerun-if-changed=native/macos.m");
        println!("cargo:rerun-if-changed=native/cpu_governor.c");
        cc::Build::new().file("native/cpu_governor.c").compile("roalt_cpu");
        let helper=std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("cpu-governor-test");
        let ok=cc::Build::new().get_compiler().to_command().args(["-DROALT_STANDALONE", "native/cpu_governor.c", "-o"]).arg(&helper).status().unwrap();
        assert!(ok.success(), "CPU helper test binary failed to compile");
        println!("cargo:rustc-env=ROALT_CPU_TEST_HELPER={}",helper.display());
        println!("cargo:rerun-if-changed=native/windows.m");
        println!("cargo:rustc-link-arg=-Wl,-weak_framework,ScreenCaptureKit");
        // Rust uses -nodefaultlibs; Clang's @available runtime guard needs its builtins archive.
        let runtime=cc::Build::new().get_compiler().to_command().arg("--print-file-name=libclang_rt.osx.a").output().expect("Clang runtime path");
        assert!(runtime.status.success());
        let runtime=String::from_utf8(runtime.stdout).unwrap();
        assert!(std::path::Path::new(runtime.trim()).is_file(), "Clang macOS runtime archive missing");
        println!("cargo:rustc-link-arg={}",runtime.trim());
        println!("cargo:rerun-if-changed=native/afk.m");
        println!("cargo:rerun-if-changed=native/install_cleanup.m");
        println!("cargo:rustc-link-lib=framework=Security");
        cc::Build::new().file("native/install_cleanup.m").file("native/afk.m").file("native/windows.m").file("native/macos.m").flag("-fobjc-arc").compile("roalt_macos");
        println!("cargo:rustc-link-lib=framework=Cocoa");
        println!("cargo:rustc-link-lib=framework=LocalAuthentication");
        println!("cargo:rustc-link-lib=framework=IOKit");
        println!("cargo:rustc-link-lib=framework=ApplicationServices");
    }
    tauri_build::build()
}
