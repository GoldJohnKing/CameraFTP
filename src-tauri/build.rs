fn main() {
    compress_lut_files();
    compress_lensfun_db();

    let mut attributes = tauri_build::Attributes::new();

    if is_windows_msvc_target() {
        attributes = attributes
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
        add_manifest_for_all_artifacts();
    }

    tauri_build::try_build(attributes).expect("failed to run tauri-build");
}

fn compress_lut_files() {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::fs;
    use std::io::{Read, Write};

    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR not set"));
    let luts_out = out_dir.join("luts");
    fs::create_dir_all(&luts_out).expect("Failed to create luts output dir");

    let luts_src = std::path::Path::new("resources/luts");
    if !luts_src.exists() {
        println!("cargo:warning=LUT source directory not found: resources/luts");
        return;
    }

    let entries = fs::read_dir(luts_src).expect("Failed to read LUT source directory");
    for entry in entries {
        let entry = entry.expect("Failed to read dir entry");
        let path = entry.path();
        if path.extension().map(|e| e == "cube").unwrap_or(false) {
            let file_name = path.file_name().unwrap();
            println!("cargo:rerun-if-changed={}", path.display());

            let mut input = fs::File::open(&path).expect("Failed to open LUT file");
            let mut data = Vec::new();
            input.read_to_end(&mut data).expect("Failed to read LUT file");

            let output_path = luts_out.join(format!("{}.gz", file_name.to_string_lossy()));
            let output = fs::File::create(&output_path).expect("Failed to create compressed file");
            let mut encoder = GzEncoder::new(output, Compression::best());
            encoder.write_all(&data).expect("Failed to compress LUT file");
            encoder.finish().expect("Failed to finish compression");
        }
    }
}

fn is_windows_msvc_target() -> bool {
    std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
}

fn add_manifest_for_all_artifacts() {
    let manifest = std::env::current_dir()
        .expect("failed to determine build script current directory")
        .join("windows-app-manifest.xml");

    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    println!("cargo:rustc-link-arg=/WX");
}

fn compress_lensfun_db() {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::collections::hash_map::DefaultHasher;
    use std::fs;
    use std::hash::{Hash, Hasher};
    use std::io::{Read, Write};

    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR not set"));
    let db_out = out_dir.join("lensfun_db");
    fs::create_dir_all(&db_out).expect("Failed to create lensfun_db output dir");

    let db_src = std::path::Path::new("resources/lensfun_db");
    if !db_src.exists() {
        println!("cargo:warning=Lensfun DB source directory not found: resources/lensfun_db");
        write_empty_manifest(&out_dir);
        return;
    }

    // Collect and sort XML files for deterministic output
    let mut entries: Vec<_> = fs::read_dir(db_src)
        .expect("Failed to read lensfun_db source directory")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|ext| ext == "xml").unwrap_or(false))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    if entries.is_empty() {
        println!("cargo:warning=No XML files found in resources/lensfun_db");
        write_empty_manifest(&out_dir);
        return;
    }

    let mut hasher = DefaultHasher::new();
    let mut manifest_lines: Vec<String> = Vec::new();

    for entry in &entries {
        let path = entry.path();
        let file_name = path.file_name().unwrap().to_string_lossy().into_owned();
        println!("cargo:rerun-if-changed={}", path.display());

        let mut input = fs::File::open(&path).expect("Failed to open XML file");
        let mut data = Vec::new();
        input.read_to_end(&mut data).expect("Failed to read XML file");

        // Hash filename + content for change detection
        file_name.hash(&mut hasher);
        data.hash(&mut hasher);

        // Gzip-compress
        let gz_name = format!("{}.gz", file_name);
        let output_path = db_out.join(&gz_name);
        let output = fs::File::create(&output_path).expect("Failed to create compressed file");
        let mut encoder = GzEncoder::new(output, Compression::best());
        encoder.write_all(&data).expect("Failed to compress XML file");
        encoder.finish().expect("Failed to finish compression");

        // Generate manifest entry: ("filename.xml", include_bytes!("lensfun_db/filename.xml.gz"))
        manifest_lines.push(format!(
            "    (\"{}\", include_bytes!(concat!(env!(\"OUT_DIR\"), \"/lensfun_db/{}\"))),",
            file_name, gz_name
        ));
    }

    let hash = format!("{:016x}", hasher.finish());

    // Generate manifest Rust file
    let manifest_content = format!(
        "/// Auto-generated by build.rs — DO NOT EDIT\n\
         /// Content hash of all embedded Lensfun DB XML files.\n\
         pub const LENSFUN_DB_HASH: &str = \"{}\";\n\
         \n\
         /// Embedded Lensfun DB XML files: (filename, compressed_data)\n\
         pub static LENSFUN_DB_FILES: &[(&str, &[u8])] = &[\n\
         {}\n\
         ];\n",
        hash,
        manifest_lines.join("\n")
    );

    let manifest_path = out_dir.join("lensfun_db_manifest.rs");
    fs::write(&manifest_path, manifest_content).expect("Failed to write manifest");
}

fn write_empty_manifest(out_dir: &std::path::Path) {
    let content = "/// Auto-generated by build.rs — DO NOT EDIT\n\
         /// No Lensfun DB XML files were found.\n\
         pub const LENSFUN_DB_HASH: &str = \"\";\n\
         \n\
         pub static LENSFUN_DB_FILES: &[(&str, &[u8])] = &[];\n";
    let manifest_path = out_dir.join("lensfun_db_manifest.rs");
    std::fs::write(&manifest_path, content).expect("Failed to write empty manifest");
}
