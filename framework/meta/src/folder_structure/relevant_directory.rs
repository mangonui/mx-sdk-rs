use crate::version::FrameworkVersion;
use multiversx_sc_meta_lib::cargo_toml::{CargoTomlContents, DependencyReference};
use std::{
    fs::{self, DirEntry},
    path::{Path, PathBuf},
};

/// Used for retrieving crate versions.
pub const FRAMEWORK_CRATE_NAMES: &[&str] = &[
    "multiversx-sc",
    "multiversx-sc-meta",
    "multiversx-sc-meta-lib",
    "multiversx-sc-scenario",
    "multiversx-sc-snippets",
    "multiversx-sc-wasm-adapter",
    "multiversx-sc-modules",
    "elrond-wasm",
    "elrond-wasm-debug",
    "elrond-wasm-modules",
    "elrond-wasm-node",
    "elrond-interact-snippets",
];

pub const CARGO_TOML_FILE_NAME: &str = "Cargo.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectoryType {
    Contract,
    Lib,
}

#[derive(Debug, Clone)]
pub struct RelevantDirectory {
    pub path: PathBuf,
    pub version: DependencyReference,
    pub upgrade_in_progress: Option<(FrameworkVersion, FrameworkVersion)>,
    pub dir_type: DirectoryType,
}

impl RelevantDirectory {
    pub fn dir_name(&self) -> String {
        self.path.file_name().unwrap().to_str().unwrap().to_string()
    }

    pub fn dir_name_underscores(&self) -> String {
        self.dir_name().replace('-', "_")
    }

    /// Gets the local meta path.
    pub fn meta_path(&self) -> PathBuf {
        self.path.join("meta")
    }

    /// Panics if meta crate path is missing.
    pub fn assert_meta_path_exists(&self) {
        let meta_path = self.meta_path();
        assert!(
            meta_path.exists(),
            "Contract meta crate not found at {}",
            meta_path.as_path().display()
        );
    }

    /// Gets the local output path.
    pub fn output_path(&self) -> PathBuf {
        self.path.join("output")
    }
}

pub struct RelevantDirectories(pub(crate) Vec<RelevantDirectory>);

impl RelevantDirectories {
    pub fn find_all(path_ref: &Path, ignore: &[String]) -> Self {
        let canonicalized = fs::canonicalize(path_ref).unwrap_or_else(|err| {
            panic!(
                "error canonicalizing input path {}: {}",
                path_ref.display(),
                err,
            )
        });
        let mut dirs = Vec::new();
        populate_directories(canonicalized.as_path(), ignore, &mut dirs);
        RelevantDirectories(dirs)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[allow(dead_code)]
    pub fn iter(&self) -> impl Iterator<Item = &RelevantDirectory> {
        self.0.iter()
    }

    pub fn iter_contract_crates(&self) -> impl Iterator<Item = &RelevantDirectory> {
        self.0
            .iter()
            .filter(|dir| dir.dir_type == DirectoryType::Contract)
    }

    pub fn count_for_version(&self, version: &FrameworkVersion) -> usize {
        self.0
            .iter()
            .filter(|dir| dir.version.eq_framework_version(version))
            .count()
    }

    pub fn iter_version(
        &mut self,
        version: &'static FrameworkVersion,
    ) -> impl Iterator<Item = &RelevantDirectory> {
        self.0
            .iter()
            .filter(move |dir| dir.version.eq_framework_version(version))
    }

    /// Marks all appropriate directories as ready for upgrade.
    pub fn start_upgrade(&mut self, from_version: FrameworkVersion, to_version: FrameworkVersion) {
        for dir in self.0.iter_mut() {
            if dir.version.eq_framework_version(&from_version) {
                dir.upgrade_in_progress = Some((from_version.clone(), to_version.clone()));
            }
        }
    }

    /// Updates the versions of all directories being upgraded (in memory)
    /// and resets upgrade status.
    pub fn finish_upgrade(&mut self) {
        for dir in self.0.iter_mut() {
            if let Some((_, to_version)) = &dir.upgrade_in_progress {
                if let DependencyReference::Version(version_req) = &mut dir.version {
                    version_req.semver = to_version.clone();
                }
                dir.upgrade_in_progress = None;
            }
        }
    }
}

fn populate_directories(path: &Path, ignore: &[String], result: &mut Vec<RelevantDirectory>) {
    let is_contract = is_marked_contract_crate_dir(path);

    if !is_contract && path.is_dir() {
        let read_dir = fs::read_dir(path).expect("error reading directory");
        for child_result in read_dir {
            let child = child_result.unwrap();
            if can_continue_recursion(&child, ignore) {
                populate_directories(child.path().as_path(), ignore, result);
            }
        }
    }

    if let Some(version) = find_framework_dependency(path) {
        let dir_type = if is_contract {
            DirectoryType::Contract
        } else {
            DirectoryType::Lib
        };
        result.push(RelevantDirectory {
            path: path.to_owned(),
            version,
            upgrade_in_progress: None,
            dir_type,
        });
    }
}

fn is_marked_contract_crate_dir(path: &Path) -> bool {
    path.join("multiversx.json").is_file() || path.join("elrond.json").is_file()
}

fn can_continue_recursion(dir_entry: &DirEntry, blacklist: &[String]) -> bool {
    if !dir_entry.file_type().unwrap().is_dir() {
        return false;
    }

    if let Some(dir_name_str) = dir_entry.file_name().to_str() {
        if blacklist.iter().any(|ignored| ignored == dir_name_str) {
            return false;
        }

        // do not explore hidden folders
        !dir_name_str.starts_with('.')
    } else {
        false
    }
}

fn load_cargo_toml_contents(dir_path: &Path) -> Option<CargoTomlContents> {
    let cargo_toml_path = dir_path.join(CARGO_TOML_FILE_NAME);
    if cargo_toml_path.is_file() {
        Some(CargoTomlContents::load_from_file(cargo_toml_path))
    } else {
        None
    }
}

impl RelevantDirectory {
    #[allow(unused)]
    pub fn cargo_toml_contents(&self) -> Option<CargoTomlContents> {
        load_cargo_toml_contents(self.path.as_path())
    }
}

fn find_framework_dependency(dir_path: &Path) -> Option<DependencyReference> {
    if let Some(cargo_toml_contents) = load_cargo_toml_contents(dir_path) {
        for &crate_name in FRAMEWORK_CRATE_NAMES {
            if let Some(dep_raw) = cargo_toml_contents.dependency_raw_value(crate_name) {
                return Some(dep_raw.interpret());
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{DirectoryType, RelevantDirectories};
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    struct TestWorkspace {
        path: PathBuf,
    }

    impl TestWorkspace {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time went backwards")
                .as_nanos();
            let path = std::env::temp_dir().join(format!("mx-sdk-rs-meta-{unique}"));
            fs::create_dir_all(&path).expect("failed to create test workspace");

            Self { path }
        }

        fn contract_dir(&self, relative_path: &str) -> PathBuf {
            self.path.join(relative_path)
        }

        fn create_contract_layout(&self, relative_path: &str, marked_contract: bool) {
            let contract_dir = self.contract_dir(relative_path);
            fs::create_dir_all(contract_dir.join("meta")).expect("failed to create meta dir");
            fs::write(
                contract_dir.join("Cargo.toml"),
                r#"[package]
name = "test-contract"
version = "0.1.0"
edition = "2021"

[dependencies]
multiversx-sc = "0.65.0"
"#,
            )
            .expect("failed to write Cargo.toml");

            if marked_contract {
                fs::write(
                    contract_dir.join("multiversx.json"),
                    "{\n  \"language\": \"rust\"\n}\n",
                )
                .expect("failed to write multiversx.json");
            }
        }
    }

    impl Drop for TestWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn contains_contract_path(dirs: &RelevantDirectories, expected_path: &Path) -> bool {
        dirs.iter_contract_crates()
            .any(|dir| dir.dir_type == DirectoryType::Contract && dir.path == expected_path)
    }

    #[test]
    fn find_all_marks_only_directories_with_multiversx_json_as_contracts() {
        let workspace = TestWorkspace::new();
        workspace.create_contract_layout("contracts/drwa/asset-manager", true);
        workspace.create_contract_layout("contracts/drwa/common", false);

        let dirs = RelevantDirectories::find_all(&workspace.path, &["target".to_string()]);

        assert!(contains_contract_path(
            &dirs,
            &workspace.contract_dir("contracts/drwa/asset-manager")
        ));
        assert!(!contains_contract_path(
            &dirs,
            &workspace.contract_dir("contracts/drwa/common")
        ));
    }
}
