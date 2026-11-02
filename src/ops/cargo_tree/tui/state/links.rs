//! Open external links for the selected dependency.

use super::*;

impl TuiState {
    pub(super) fn open_selected_dependency(&self) {
        let Some(dependency) =
            Self::selected_dependency(&self.dependency_tree, &self.tree_widget_state)
        else {
            return;
        };
        let Some(package_id) = dependency.package_id else {
            return;
        };
        let source_id = package_id.source_id();
        if source_id.is_path() {
            return;
        }

        let url = format!(
            "https://crates.io/crates/{}/{}",
            dependency.name, dependency.version
        );
        let _ = open_url(&url);
    }

    pub(super) fn open_selected_docs_rs(&self) {
        let Some(dependency) =
            Self::selected_dependency(&self.dependency_tree, &self.tree_widget_state)
        else {
            return;
        };
        let Some(package_id) = dependency.package_id else {
            return;
        };
        if package_id.source_id().is_path() {
            return;
        }

        let url = format!("https://docs.rs/{}/{}", dependency.name, dependency.version);
        let _ = open_url(&url);
    }
}

fn open_url(url: &str) -> std::io::Result<()> {
    #[cfg(any(unix, target_os = "windows"))]
    {
        #[cfg(target_os = "macos")]
        let mut command = Command::new("open");
        #[cfg(target_os = "windows")]
        let mut command = Command::new("explorer.exe");
        #[cfg(all(unix, not(target_os = "macos")))]
        let mut command = Command::new("xdg-open");

        command
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(|_| ())
    }
    #[cfg(not(any(unix, target_os = "windows")))]
    {
        let _ = url;
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "opening URLs is not supported on this platform",
        ))
    }
}
