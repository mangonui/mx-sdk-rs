use multiversx_sc_meta::cmd::install::install_debugger;
use multiversx_sc_meta_lib::tools::find_current_workspace;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

const INSTALL_DEBUGGER_TEMP_DIR_NAME: &str = "install-debugger-test";

fn is_writable_or_creatable(path: &PathBuf) -> bool {
    match fs::create_dir_all(path) {
        Ok(()) => true,
        Err(err) => !matches!(
            err.kind(),
            ErrorKind::PermissionDenied | ErrorKind::ReadOnlyFilesystem
        ),
    }
}

#[tokio::test]
async fn test_install_debugger() {
    if tokio::net::lookup_host(("github.com", 443)).await.is_err() {
        eprintln!("skipping install_debugger test: github.com is not resolvable");
        return;
    }

    let home_dir = match std::env::var_os("HOME") {
        Some(path) => PathBuf::from(path),
        None => {
            eprintln!("skipping install_debugger test: HOME is not set");
            return;
        }
    };
    let vscode_logs_dir = home_dir.join(".config/Code/logs");
    let vscode_extensions_dir = home_dir.join(".vscode/extensions");
    if !is_writable_or_creatable(&vscode_logs_dir)
        || !is_writable_or_creatable(&vscode_extensions_dir)
    {
        eprintln!(
            "skipping install_debugger test: VS Code directories are not writable in this environment"
        );
        return;
    }

    let workspace_path = find_current_workspace().unwrap();
    let target_path = workspace_path.join(INSTALL_DEBUGGER_TEMP_DIR_NAME);
    install_debugger::install_debugger(Option::Some(target_path)).await;
}
