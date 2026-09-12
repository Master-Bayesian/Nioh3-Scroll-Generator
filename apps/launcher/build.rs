use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=../../assets/nioh3-scroll-generator.ico");
    println!("cargo:rerun-if-changed=launcher.manifest");
    println!("cargo:rerun-if-changed=build.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let icon = root
        .join("../../assets/nioh3-scroll-generator.ico")
        .canonicalize()
        .unwrap();
    let manifest = root.join("launcher.manifest");
    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let numbers = format!("{},0", version.replace('.', ","));
    let resource = out.join("launcher.rc");
    fs::write(
        &resource,
        format!(
            r#"
1 ICON "{}"
1 24 "{}"
1 VERSIONINFO
FILEVERSION {numbers}
PRODUCTVERSION {numbers}
FILEFLAGSMASK 0x3fL
FILEOS 0x40004L
FILETYPE 0x1L
BEGIN
 BLOCK "StringFileInfo"
 BEGIN
  BLOCK "040904b0"
  BEGIN
   VALUE "FileDescription", "Nioh 3 Studio portable launcher\0"
   VALUE "FileVersion", "{version}\0"
   VALUE "ProductName", "Nioh 3 Studio\0"
   VALUE "ProductVersion", "{version}\0"
  END
 END
 BLOCK "VarFileInfo"
 BEGIN
  VALUE "Translation", 0x0409, 1200
 END
END
"#,
            icon.to_string_lossy().replace('\\', "/"),
            manifest.to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
    embed_resource::compile(resource, embed_resource::NONE)
        .manifest_required()
        .unwrap();
}
