fn main() {
    let base = "vendor/rcheevos";
    println!("cargo:rerun-if-changed={base}");
    println!("cargo:rerun-if-changed=src/runtime.c");
    let mut build = cc::Build::new();
    build
        .include(format!("{base}/include"))
        .include(format!("{base}/src"));
    build.define("RC_DISABLE_LUA", None).warnings(false);
    for entry in std::fs::read_dir(format!("{base}/src/rcheevos")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|s| s == "c") {
            build.file(path);
        }
    }
    build.file(format!("{base}/src/rc_compat.c"));
    build.file(format!("{base}/src/rc_util.c"));
    build.file(format!("{base}/src/rhash/md5.c"));
    build.file("src/runtime.c").compile("slot_rcheevos");
}
