fn main() {
    pack_lut_zip();
    compress_lensfun_db();
    compress_raw_alchemy_dll();
    compress_libomp_dll();
    compress_onnxruntime_dll();
    compress_directml_dll();

    let mut attributes = tauri_build::Attributes::new();

    if is_windows_msvc_target() {
        attributes = attributes
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
        add_manifest_for_all_artifacts();
    }

    tauri_build::try_build(attributes).expect("tauri-build failed — check that tauri.conf.json is valid and all referenced icons exist");
}

fn pack_lut_zip() {
    use std::fs;
    use std::io::{Read, Write};

    let luts_src = std::path::Path::new("resources/luts");
    if !luts_src.exists() {
        println!("cargo:warning=LUT source directory not found: resources/luts");
        return;
    }

    // Collect and sort .cube files for deterministic output
    let mut entries: Vec<_> = fs::read_dir(luts_src)
        .expect("Failed to read LUT source directory 'resources/luts' — ensure the directory exists and is readable")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|ext| ext == "cube").unwrap_or(false))
        .collect();
    entries.sort_by_key(|e| e.file_name());

    if entries.is_empty() {
        println!("cargo:warning=No .cube files found in resources/luts");
        return;
    }

    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR env var not set — this should be provided by Cargo; are you running outside of 'cargo build'?"));
    let zip_path = out_dir.join("luts.zip");

    let zip_file = fs::File::create(&zip_path).expect("Failed to create luts.zip in OUT_DIR — check disk space and write permissions");
    let mut zip_writer = zip::ZipWriter::new(zip_file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .compression_level(Some(9));

    for entry in &entries {
        let path = entry.path();
        let file_name = path.file_name().unwrap().to_string_lossy().into_owned();
        println!("cargo:rerun-if-changed={}", path.display());

        let mut data = Vec::new();
        let mut input = fs::File::open(&path).expect("Failed to open LUT .cube file — check file permissions");
        input.read_to_end(&mut data).expect("Failed to read LUT .cube file — file may be corrupted or unreadable");

        zip_writer
            .start_file(&file_name, options)
            .expect("Failed to start ZIP entry for LUT file — ZIP writer may be in an invalid state");
        zip_writer.write_all(&data).expect("Failed to write LUT data to ZIP entry — check disk space");
    }

    zip_writer.finish().expect("Failed to finalize ZIP archive — check disk space and write permissions");

    let compressed_size = fs::metadata(&zip_path).map(|m| m.len()).unwrap_or(0);
    println!(
        "cargo:warning=Packed {} LUT files into luts.zip ({} KB)",
        entries.len(),
        compressed_size / 1024
    );
}

fn is_windows_msvc_target() -> bool {
    std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
}

fn add_manifest_for_all_artifacts() {
    let manifest = std::env::current_dir()
        .expect("Failed to determine build script current directory — check that the working directory is accessible")
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

    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR env var not set — this should be provided by Cargo; are you running outside of 'cargo build'?"));
    let db_out = out_dir.join("lensfun_db");
    fs::create_dir_all(&db_out).expect("Failed to create lensfun_db output directory in OUT_DIR — check disk space and write permissions");

    let db_src = std::path::Path::new("resources/lensfun_db");
    if !db_src.exists() {
        println!("cargo:warning=Lensfun DB source directory not found: resources/lensfun_db");
        write_empty_manifest(&out_dir);
        return;
    }

    // Collect and sort XML files for deterministic output
    let mut entries: Vec<_> = fs::read_dir(db_src)
        .expect("Failed to read lensfun_db source directory 'resources/lensfun_db' — ensure the directory exists and is readable")
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

        let mut input = fs::File::open(&path).expect("Failed to open Lensfun DB XML file — check file permissions");
        let mut data = Vec::new();
        input.read_to_end(&mut data).expect("Failed to read Lensfun DB XML file — file may be corrupted or unreadable");

        // Hash filename + content for change detection
        file_name.hash(&mut hasher);
        data.hash(&mut hasher);

        // Gzip-compress
        let gz_name = format!("{}.gz", file_name);
        let output_path = db_out.join(&gz_name);
        let output = fs::File::create(&output_path).expect("Failed to create compressed .gz file in OUT_DIR/lensfun_db — check disk space and write permissions");
        let mut encoder = GzEncoder::new(output, Compression::best());
        encoder.write_all(&data).expect("Failed to compress XML file to gzip — check disk space");
        encoder.finish().expect("Failed to finish gzip compression — check disk space and write permissions");

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
    fs::write(&manifest_path, manifest_content).expect("Failed to write lensfun_db_manifest.rs to OUT_DIR — check disk space and write permissions");
}

fn write_empty_manifest(out_dir: &std::path::Path) {
    let content = "/// Auto-generated by build.rs — DO NOT EDIT\n\
         /// No Lensfun DB XML files were found.\n\
         pub const LENSFUN_DB_HASH: &str = \"\";\n\
         \n\
         pub static LENSFUN_DB_FILES: &[(&str, &[u8])] = &[];\n";
    let manifest_path = out_dir.join("lensfun_db_manifest.rs");
    std::fs::write(&manifest_path, content).expect("Failed to write empty lensfun_db_manifest.rs to OUT_DIR — check disk space and write permissions");
}

/// Gzip-compress raw_alchemy_core.dll into OUT_DIR for embedding via include_bytes!.
/// The DLL is built by CMake before cargo builds, so it should already exist on disk.
fn compress_raw_alchemy_dll() {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::fs;
    use std::io::{Read, Write};

    if !is_windows_msvc_target() {
        return;
    }

    let rawalchemy_dir = std::path::Path::new("lib/rawalchemy");
    if !rawalchemy_dir.exists() {
        println!("cargo:warning=RawAlchemyCpp not found, skipping DLL embedding");
        write_empty_dll_placeholder("raw_alchemy_core.dll.gz");
        return;
    }

    // Re-run build.rs when the CMake definition changes (so an edited CMakeLists
    // or .def triggers re-embedding even if the built DLL timestamp is unchanged).
    println!("cargo:rerun-if-changed=lib/rawalchemy/CMakeLists.txt");
    println!("cargo:rerun-if-changed=lib/rawalchemy/raw_alchemy_exports.def");

    let build_type = if std::env::var("PROFILE").as_deref() == Ok("debug") {
        "Debug"
    } else {
        "Release"
    };

    let dll_path = rawalchemy_dir
        .join("build-windows-dll")
        .join("bin")
        .join(build_type)
        .join("raw_alchemy_core.dll");

    if !dll_path.exists() {
        println!(
            "cargo:warning=raw_alchemy_core.dll not found at {}, skipping DLL embedding",
            dll_path.display()
        );
        write_empty_dll_placeholder("raw_alchemy_core.dll.gz");
        return;
    }

    println!("cargo:rerun-if-changed={}", dll_path.display());

    let mut input = fs::File::open(&dll_path).expect("Failed to open raw_alchemy_core.dll — ensure the CMake build completed successfully and the DLL exists at the expected path");
    let mut data = Vec::new();
    input.read_to_end(&mut data).expect("Failed to read raw_alchemy_core.dll — file may be locked by another process or corrupted");

    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR env var not set — this should be provided by Cargo; are you running outside of 'cargo build'?"));
    let output_path = out_dir.join("raw_alchemy_core.dll.gz");
    let output = fs::File::create(&output_path).expect("Failed to create compressed DLL file in OUT_DIR — check disk space and write permissions");
    let mut encoder = GzEncoder::new(output, Compression::best());
    encoder.write_all(&data).expect("Failed to compress DLL to gzip — check disk space");
    encoder.finish().expect("Failed to finish DLL gzip compression — check disk space and write permissions");

    let compressed_size = fs::metadata(&output_path).map(|m| m.len()).unwrap_or(0);
    println!(
        "cargo:warning=Embedded raw_alchemy_core.dll: {} KB → {} KB (gzip)",
        data.len() / 1024,
        compressed_size / 1024
    );
}

fn write_empty_dll_placeholder(filename: &str) {
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR env var not set — this should be provided by Cargo; are you running outside of 'cargo build'?"));
    let placeholder = out_dir.join(filename);
    // Write a minimal gzip file (empty payload) so include_bytes! still compiles
    use std::io::Write;
    let mut f = std::fs::File::create(&placeholder).expect("Failed to create DLL placeholder file in OUT_DIR — check disk space and write permissions");
    // Minimal gzip: 10-byte header + 8-byte footer for empty content
    let empty_gz: &[u8] = &[
        0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x03, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
    ];
    f.write_all(empty_gz).expect("Failed to write DLL placeholder content — check disk space and write permissions");
}

/// Gzip-compress libomp.dll (the OpenMP runtime, copied next to
/// raw_alchemy_core.dll by the CMake POST_BUILD step) into OUT_DIR for embedding.
/// raw_alchemy_core.dll has a load-time dependency on libomp.dll; the host
/// preloads it before LoadLibrary-ing the core DLL so the dependency resolves
/// without libomp needing to be on the system PATH or in System32.
fn compress_libomp_dll() {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::fs;
    use std::io::{Read, Write};

    if !is_windows_msvc_target() {
        return;
    }

    let rawalchemy_dir = std::path::Path::new("lib/rawalchemy");
    if !rawalchemy_dir.exists() {
        write_empty_dll_placeholder("libomp.dll.gz");
        return;
    }

    let build_type = if std::env::var("PROFILE").as_deref() == Ok("debug") {
        "Debug"
    } else {
        "Release"
    };

    let libomp_path = rawalchemy_dir
        .join("build-windows-dll")
        .join("bin")
        .join(build_type)
        .join("libomp.dll");

    if !libomp_path.exists() {
        println!(
            "cargo:warning=libomp.dll not found at {}, skipping libomp embedding",
            libomp_path.display()
        );
        write_empty_dll_placeholder("libomp.dll.gz");
        return;
    }

    println!("cargo:rerun-if-changed={}", libomp_path.display());

    let mut input = fs::File::open(&libomp_path)
        .expect("Failed to open libomp.dll — ensure the CMake POST_BUILD copy completed");
    let mut data = Vec::new();
    input
        .read_to_end(&mut data)
        .expect("Failed to read libomp.dll — file may be locked by another process or corrupted");

    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR env var not set — this should be provided by Cargo; are you running outside of 'cargo build'?"));
    let output_path = out_dir.join("libomp.dll.gz");
    let output = fs::File::create(&output_path)
        .expect("Failed to create compressed libomp.dll file in OUT_DIR — check disk space and write permissions");
    let mut encoder = GzEncoder::new(output, Compression::best());
    encoder
        .write_all(&data)
        .expect("Failed to compress libomp.dll to gzip — check disk space");
    encoder
        .finish()
        .expect("Failed to finish libomp.dll gzip compression — check disk space and write permissions");

    let compressed_size = fs::metadata(&output_path).map(|m| m.len()).unwrap_or(0);
    println!(
        "cargo:warning=Embedded libomp.dll: {} KB → {} KB (gzip)",
        data.len() / 1024,
        compressed_size / 1024
    );
}

/// Gzip a source DLL into `<OUT_DIR>/<out_name>` for embedding via include_bytes!.
/// Shared by the ORT/DirectML embeds (both pulled from the nn-cache, not built
/// locally). Emits a placeholder on Windows when the source is absent so the
/// downstream include_bytes! still compiles.
fn compress_nn_cache_dll(
    src_path: std::path::PathBuf,
    out_name: &str,
    label: &str,
    rerun_if: &str,
) {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::fs;
    use std::io::{Read, Write};

    if !is_windows_msvc_target() {
        return;
    }

    println!("cargo:rerun-if-changed={}", rerun_if);

    if !src_path.exists() {
        println!(
            "cargo:warning={} not found at {}, skipping embed (NN runtime will be unavailable)",
            label,
            src_path.display()
        );
        write_empty_dll_placeholder(out_name);
        return;
    }

    let mut input = match fs::File::open(&src_path) {
        Ok(f) => f,
        Err(e) => {
            println!(
                "cargo:warning=Failed to open {} at {}: {} — skipping embed",
                label,
                src_path.display(),
                e
            );
            write_empty_dll_placeholder(out_name);
            return;
        }
    };
    let mut data = Vec::new();
    if let Err(e) = input.read_to_end(&mut data) {
        println!(
            "cargo:warning=Failed to read {} ({}): {} — skipping embed",
            label,
            src_path.display(),
            e
        );
        write_empty_dll_placeholder(out_name);
        return;
    }

    let out_dir = std::path::PathBuf::from(
        std::env::var("OUT_DIR").expect("OUT_DIR env var not set — this should be provided by Cargo; are you running outside of 'cargo build'?"),
    );
    let output_path = out_dir.join(out_name);
    let output = fs::File::create(&output_path).unwrap_or_else(|e| {
        panic!(
            "Failed to create compressed {} file in OUT_DIR ({}): {}",
            label, out_dir.display(), e
        )
    });
    let mut encoder = GzEncoder::new(output, Compression::best());
    encoder
        .write_all(&data)
        .unwrap_or_else(|e| panic!("Failed to compress {} to gzip: {}", label, e));
    encoder
        .finish()
        .unwrap_or_else(|e| panic!("Failed to finish {} gzip compression: {}", label, e));

    let compressed_size = fs::metadata(&output_path).map(|m| m.len()).unwrap_or(0);
    println!(
        "cargo:warning=Embedded {}: {} KB → {} KB (gzip)",
        label,
        data.len() / 1024,
        compressed_size / 1024
    );
}

/// Gzip the DirectML-capable onnxruntime.dll (from the nn-cache after
/// fetch-nn-deps.sh) into OUT_DIR for embedding. This is the ORT build with the
/// DirectML EP compiled in — without it, GetExecutionProviderApi("DML") returns
/// null and NN init always fails. raw_alchemy_core.dll imports onnxruntime.dll,
/// so the host preloads it before loading the core DLL.
fn compress_onnxruntime_dll() {
    if !is_windows_msvc_target() {
        return;
    }
    // Re-run when the fetch script changes so a version bump re-embeds.
    println!("cargo:rerun-if-changed=../scripts/fetch-nn-deps.sh");

    let src = std::path::Path::new("lib/rawalchemy")
        .join("third_party/nn-cache")
        .join("onnxruntime-win-x64-1.24.1")
        .join("lib/onnxruntime.dll");
    compress_nn_cache_dll(
        src,
        "onnxruntime.dll.gz",
        "onnxruntime.dll (DirectML)",
        "lib/rawalchemy/third_party/nn-cache/onnxruntime-win-x64-1.24.1/lib/onnxruntime.dll",
    );
}

/// Gzip DirectML.dll (from the Microsoft.AI.DirectML NuGet via fetch-nn-deps.sh)
/// into OUT_DIR for embedding. ORT's DirectML EP loads DirectML.dll at runtime
/// (DMLCreateDevice); the host preloads it so the EP resolves our copy rather
/// than a stale System32 version.
fn compress_directml_dll() {
    if !is_windows_msvc_target() {
        return;
    }
    println!("cargo:rerun-if-changed=../scripts/fetch-nn-deps.sh");

    let src = std::path::Path::new("lib/rawalchemy")
        .join("third_party/nn-cache/DirectML.dll");
    compress_nn_cache_dll(
        src,
        "directml.dll.gz",
        "DirectML.dll",
        "lib/rawalchemy/third_party/nn-cache/DirectML.dll",
    );
}
