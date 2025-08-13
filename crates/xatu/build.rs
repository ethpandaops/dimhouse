use std::env;
use std::fs;
use std::io::Read;
use std::path::Path;

// Version of xatu-sidecar to download from GitHub releases
// Update this when new versions are released: https://github.com/ethpandaops/xatu-sidecar/releases
const XATU_SIDECAR_VERSION: &str = "v0.0.4";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let lib_dir = Path::new(&manifest_dir).join("src");
    let lib_path = lib_dir.join("libxatu.so");

    // Check if we need to download the library
    if !lib_path.exists() || should_update_library(&lib_path) {
        download_xatu_sidecar(&lib_dir)?;
    }

    // Tell cargo where to find the library
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=xatu");

    // Copy the library to the output directory
    let out_dir = env::var("OUT_DIR").unwrap();
    let profile = env::var("PROFILE").unwrap();
    let target_dir = Path::new(&out_dir)
        .ancestors()
        .find(|p| p.ends_with(&profile))
        .unwrap()
        .parent()
        .unwrap();

    let lib_file = lib_dir.join("libxatu.so");
    let dest_file = target_dir.join(&profile).join("libxatu.so");

    if lib_file.exists() {
        std::fs::copy(&lib_file, &dest_file)
            .expect("Failed to copy libxatu.so to output directory");
    }

    // Set rpath to look in the same directory as the binary
    // These need to be passed to the final binary, not just this crate
    println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN");
    println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN/../lib");
    println!("cargo:rustc-link-arg=-Wl,-rpath,@loader_path");
    println!("cargo:rustc-link-arg=-Wl,-rpath,@loader_path/../lib");
    println!("cargo:rustc-link-arg=-Wl,--enable-new-dtags");

    Ok(())
}

fn should_update_library(_lib_path: &Path) -> bool {
    // For now, always use the existing library if it exists
    // In the future, we could check if a newer version is available
    false
}

fn download_xatu_sidecar(lib_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let platform = match (env::consts::OS, env::consts::ARCH) {
        ("linux", "x86_64") => "linux_amd64",
        ("linux", "aarch64") => "linux_arm64",
        ("macos", "x86_64") => "darwin_amd64",
        ("macos", "aarch64") => "darwin_arm64",
        _ => {
            return Err(format!(
                "Unsupported platform: {} {}",
                env::consts::OS,
                env::consts::ARCH
            )
            .into())
        }
    };

    let lib_name = if env::consts::OS == "macos" {
        "libxatu.dylib"
    } else {
        "libxatu.so"
    };

    let url = format!(
        "https://github.com/ethpandaops/xatu-sidecar/releases/download/{}/xatu-sidecar_{}_{}.tar.gz",
        XATU_SIDECAR_VERSION,
        XATU_SIDECAR_VERSION.trim_start_matches('v'), // Remove 'v' prefix for filename
        platform
    );

    println!(
        "cargo:warning=Downloading xatu-sidecar {} for {}",
        XATU_SIDECAR_VERSION, platform
    );

    // Download the tarball
    let response = ureq::get(&url).call()?;
    let mut data = Vec::new();
    response.into_reader().read_to_end(&mut data)?;

    // Extract the library
    let tar = flate2::read::GzDecoder::new(&data[..]);
    let mut archive = tar::Archive::new(tar);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;
        if path.file_name() == Some(std::ffi::OsStr::new(lib_name)) {
            let dest_path = lib_dir.join("libxatu.so"); // Always save as .so for consistency
            let mut dest_file = fs::File::create(&dest_path)?;
            std::io::copy(&mut entry, &mut dest_file)?;

            // Make the library executable
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&dest_path)?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&dest_path, perms)?;
            }

            println!("cargo:warning=Successfully downloaded xatu-sidecar library");
            return Ok(());
        }
    }

    Err(format!("Library {} not found in release archive", lib_name).into())
}
