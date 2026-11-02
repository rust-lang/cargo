//! Package snapshots containing the metadata needed by the tree and popup.

use super::*;

/// Snapshot of a Cargo package with the fields required by the tree.
pub struct PackageSnapshot {
    pub(super) name: String,
    pub(super) version: String,
    pub(super) package_id: PackageId,
    pub(super) manifest_dir: Option<String>,
    pub(super) is_proc_macro: bool,
    pub(super) format_metadata: Option<Arc<PackageFormatMetadata>>,
}

impl PackageSnapshot {
    pub(super) fn from_package(
        package: &Package,
        features: Option<String>,
        format_requirements: PackageFormatRequirements,
    ) -> Self {
        let package_id = package.package_id();
        let source_id = package_id.source_id();
        let manifest_dir = package
            .package_id()
            .source_id()
            .is_path()
            .then(|| package.root().display().to_string());
        let metadata = package.manifest().metadata();
        let format_metadata = PackageFormatMetadata {
            description: format_requirements
                .metadata
                .then(|| metadata.description.clone())
                .flatten(),
            edition: format_requirements
                .metadata
                .then(|| package.manifest().edition().to_string()),
            rust_version: format_requirements
                .metadata
                .then(|| package.rust_version().map(ToString::to_string))
                .flatten(),
            targets: format_requirements
                .metadata
                .then(|| summarize_targets(package.targets())),
            license: (format_requirements.license || format_requirements.metadata)
                .then(|| metadata.license.clone())
                .flatten(),
            repository: (format_requirements.repository || format_requirements.metadata)
                .then(|| metadata.repository.clone())
                .flatten(),
            homepage: format_requirements
                .metadata
                .then(|| metadata.homepage.clone())
                .flatten(),
            documentation: format_requirements
                .metadata
                .then(|| metadata.documentation.clone())
                .flatten(),
            authors: (format_requirements.metadata && !metadata.authors.is_empty())
                .then(|| metadata.authors.join(", ")),
            keywords: (format_requirements.metadata && !metadata.keywords.is_empty())
                .then(|| metadata.keywords.join(", ")),
            categories: (format_requirements.metadata && !metadata.categories.is_empty())
                .then(|| metadata.categories.join(", ")),
            features,
            library: (format_requirements.library || format_requirements.metadata)
                .then(|| {
                    package
                        .targets()
                        .iter()
                        .find(|target| target.is_lib())
                        .map(|target| target.crate_name().to_string())
                })
                .flatten(),
            source: (format_requirements.source && !source_id.is_crates_io())
                .then(|| source_id.to_string()),
        };
        let format_metadata = (format_requirements.metadata
            || format_metadata.description.is_some()
            || format_metadata.license.is_some()
            || format_metadata.repository.is_some()
            || format_metadata.homepage.is_some()
            || format_metadata.documentation.is_some()
            || format_metadata.authors.is_some()
            || format_metadata.keywords.is_some()
            || format_metadata.categories.is_some()
            || format_metadata.features.is_some()
            || format_metadata.library.is_some()
            || format_metadata.source.is_some())
        .then(|| Arc::new(format_metadata));

        Self {
            name: package.name().as_str().to_owned(),
            version: package.version().to_string(),
            package_id,
            manifest_dir,
            is_proc_macro: package.proc_macro(),
            format_metadata,
        }
    }
}

fn summarize_targets(targets: &[Target]) -> String {
    #[derive(Default)]
    struct Counts {
        library: usize,
        proc_macro: usize,
        binary: usize,
        example: usize,
        test: usize,
        bench: usize,
        build_script: usize,
    }

    let mut counts = Counts::default();
    for target in targets {
        if target.is_lib() {
            if target.proc_macro() {
                counts.proc_macro += 1;
            } else {
                counts.library += 1;
            }
        } else if target.is_bin() {
            counts.binary += 1;
        } else if target.is_example() {
            counts.example += 1;
        } else if target.is_test() {
            counts.test += 1;
        } else if target.is_bench() {
            counts.bench += 1;
        } else if target.is_custom_build() {
            counts.build_script += 1;
        }
    }

    let mut summary = String::new();
    for (label, count) in [
        ("library", counts.library),
        ("proc-macro", counts.proc_macro),
        ("binary", counts.binary),
        ("example", counts.example),
        ("test", counts.test),
        ("bench", counts.bench),
        ("build script", counts.build_script),
    ] {
        if count == 0 {
            continue;
        }
        if !summary.is_empty() {
            summary.push_str(", ");
        }
        if count == 1 {
            summary.push_str(label);
        } else {
            let _ = write!(summary, "{count} {label}s");
        }
    }

    if summary.is_empty() {
        "None".to_owned()
    } else {
        summary
    }
}
