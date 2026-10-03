fn main() {
    // Lib tests link tao's comctl32 v6 import. Bins already embed that manifest
    // via tauri-build; the lib test harness does not, and a second manifest on
    // the bin is a duplicate resource. The static lib is linked only from cfg(test).
    if std::env::var("CARGO_CFG_TARGET_OS").ok().as_deref() == Some("windows") {
        embed_windows_test_manifest();
    }

    // App ACL manifest for the commands the desktop UI invokes.
    // session_update_cwd is registered in main.rs but has no tauriInvoke
    // caller, so it stays deny-by-default and is omitted here.
    // Underscores become hyphens in the generated allow-<command> identifiers.
    let commands = &[
        "session_create",
        "session_list",
        "session_close",
        "terminal_execute",
        "terminal_interrupt",
        "terminal_resize",
        "terminal_resync",
        "terminal_write",
        "planner_generate_plan",
        "history_append",
        "history_list",
        "history_update",
        "plan_store",
        "settings_get",
        "settings_update",
        "workflow_add",
        "workflow_delete",
        "workflow_list",
        "memory_list",
        "memory_add",
        "memory_accept_suggestion",
        "memory_dismiss_suggestion",
        "memory_delete",
        "memory_store_suggestion",
        "memory_list_resolved_suggestions",
    ];

    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(commands)),
    )
    .expect("failed to run tauri-build");
}

fn embed_windows_test_manifest() {
    let manifest_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let manifest = manifest_dir.join("windows-test-manifest.xml");
    println!("cargo:rerun-if-changed={}", manifest.display());

    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let rc_path = out_dir.join("commandui_test_manifest.rc");
    let res_path = out_dir.join("commandui_test_manifest.res");
    let asm_path = out_dir.join("commandui_test_manifest.asm");
    let obj_path = out_dir.join("commandui_test_manifest.obj");
    let lib_path = out_dir.join("commandui_test_manifest.lib");
    let manifest_rc = manifest.display().to_string().replace('\\', "/");
    std::fs::write(&rc_path, format!("1 24 \"{manifest_rc}\"\n")).expect("write rc");
    std::fs::write(
        &asm_path,
        "\
.code
PUBLIC commandui_test_manifest_anchor
commandui_test_manifest_anchor PROC
    ret
commandui_test_manifest_anchor ENDP
END
",
    )
    .expect("write asm");

    let rc_status = std::process::Command::new(find_rc())
        .args([
            "/nologo",
            "/fo",
            res_path.to_str().expect("res path"),
            rc_path.to_str().expect("rc path"),
        ])
        .status()
        .expect("spawn rc.exe");
    if !rc_status.success() {
        panic!("rc.exe failed with {rc_status}");
    }

    let lib_exe = find_lib();
    let ml64 = lib_exe.with_file_name("ml64.exe");
    let ml_status = std::process::Command::new(&ml64)
        .args([
            "/nologo",
            "/c",
            "/Fo",
            obj_path.to_str().expect("obj path"),
            asm_path.to_str().expect("asm path"),
        ])
        .status()
        .expect("spawn ml64.exe");
    if !ml_status.success() {
        panic!("ml64.exe failed with {ml_status}");
    }

    let lib_status = std::process::Command::new(&lib_exe)
        .args([
            "/NOLOGO",
            &format!("/OUT:{}", lib_path.display()),
            obj_path.to_str().expect("obj path"),
            res_path.to_str().expect("res path"),
        ])
        .status()
        .expect("spawn lib.exe");
    if !lib_status.success() {
        panic!("lib.exe failed with {lib_status}");
    }

    println!("cargo:rustc-link-search=native={}", out_dir.display());
}

fn find_lib() -> std::path::PathBuf {
    if let Some(found) = find_on_path_optional("lib.exe") {
        return found;
    }
    if let Some(tools) = std::env::var_os("VCToolsInstallDir") {
        let candidate = std::path::PathBuf::from(tools)
            .join("bin")
            .join("Hostx64")
            .join("x64")
            .join("lib.exe");
        if candidate.is_file() {
            return candidate;
        }
    }
    let vswhere = std::path::Path::new(
        r"C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe",
    );
    if vswhere.is_file() {
        if let Ok(output) = std::process::Command::new(vswhere)
            .args([
                "-latest",
                "-products",
                "*",
                "-requires",
                "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
                "-find",
                r"VC\Tools\MSVC\*\bin\Hostx64\x64\lib.exe",
            ])
            .output()
        {
            if let Some(line) = String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
            {
                let candidate = std::path::PathBuf::from(line);
                if candidate.is_file() {
                    return candidate;
                }
            }
        }
    }
    panic!("lib.exe was not on PATH or under Visual Studio");
}

fn find_rc() -> std::path::PathBuf {
    if let Some(found) = find_on_path_optional("rc.exe") {
        return found;
    }
    let kits = std::path::Path::new(r"C:\Program Files (x86)\Windows Kits\10\bin");
    let mut versions = std::fs::read_dir(kits)
        .map(|entries| {
            entries
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.path())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    versions.sort();
    for version in versions.iter().rev() {
        let candidate = version.join("x64").join("rc.exe");
        if candidate.is_file() {
            return candidate;
        }
    }
    panic!("rc.exe was not on PATH or under the Windows SDK");
}

fn find_on_path_optional(name: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}
