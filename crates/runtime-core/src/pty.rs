use portable_pty::{native_pty_system, CommandBuilder, PtyPair, PtySize};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

pub const PROMPT_MARKER: &str = "__COMMANDUI_PROMPT__";

pub type PtyHandle = Arc<Mutex<Box<dyn Write + Send>>>;

pub fn default_shell() -> String {
    #[cfg(target_os = "windows")]
    {
        if let Ok(shell) = std::env::var("COMMANDUI_WINDOWS_SHELL") {
            return shell;
        }
        let pwsh7 = format!(
            "{}\\PowerShell\\7\\pwsh.exe",
            std::env::var("ProgramFiles").unwrap_or_default()
        );
        if std::path::Path::new(&pwsh7).exists() {
            return pwsh7;
        }
        "powershell.exe".to_string()
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string())
    }
}

pub fn spawn_shell(shell: &str, cwd: Option<&str>) -> Result<(PtyPair, PtyHandle), String> {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 30,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| format!("Failed to open PTY: {e}"))?;

    let mut cmd = CommandBuilder::new(shell);
    if let Some(dir) = cwd {
        cmd.cwd(dir);
    }

    pair.slave
        .spawn_command(cmd)
        .map_err(|e| format!("Failed to spawn shell: {e}"))?;

    let writer = pair
        .master
        .take_writer()
        .map_err(|e| format!("Failed to get PTY writer: {e}"))?;

    let handle: PtyHandle = Arc::new(Mutex::new(writer));

    Ok((pair, handle))
}

pub fn write_command(handle: &PtyHandle, command: &str) -> Result<(), String> {
    let mut writer = handle.lock().map_err(|e| format!("Lock error: {e}"))?;
    writer
        .write_all(format!("{command}\n").as_bytes())
        .map_err(|e| format!("Write error: {e}"))?;
    writer.flush().map_err(|e| format!("Flush error: {e}"))?;
    Ok(())
}

pub fn write_raw(handle: &PtyHandle, data: &str) -> Result<(), String> {
    let mut writer = handle.lock().map_err(|e| format!("Lock error: {e}"))?;
    writer
        .write_all(data.as_bytes())
        .map_err(|e| format!("Write error: {e}"))?;
    writer.flush().map_err(|e| format!("Flush error: {e}"))?;
    Ok(())
}

pub fn bootstrap_prompt(shell: &str) -> Option<String> {
    let shell_lower = shell.to_lowercase();

    if shell_lower.contains("pwsh") || shell_lower.contains("powershell") {
        Some(format!(
            "function prompt {{ \"{PROMPT_MARKER}|$((Get-Location).Path)|$LASTEXITCODE`n> \" }}\n"
        ))
    } else if shell_lower.contains("cmd") {
        Some(format!("prompt {PROMPT_MARKER}|$P$_$G \n"))
    } else if shell_lower.contains("bash") {
        Some(format!(
            "export PS1='{PROMPT_MARKER}|\\w|$?\\n\\$ '\n"
        ))
    } else if shell_lower.contains("zsh") {
        Some(format!(
            "export PS1='{PROMPT_MARKER}|%~|%?\\n%% '\n"
        ))
    } else {
        None
    }
}

pub fn spawn_reader_loop<F>(pair: &PtyPair, on_chunk: F)
where
    F: Fn(String) + Send + 'static,
{
    let mut reader = pair
        .master
        .try_clone_reader()
        .expect("Failed to clone PTY reader");

    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let text = String::from_utf8_lossy(&buf[..n]).to_string();
                    on_chunk(text);
                }
                Err(_) => break,
            }
        }
    });
}
