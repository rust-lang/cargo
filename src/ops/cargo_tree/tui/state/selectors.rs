//! Configure interactive tree selectors and their choices.

use super::*;

impl TuiState {
    pub(super) fn open_edge_selector(&mut self) {
        let selected = edge_kinds(&self.tree_options.resolution);
        self.selector = Some(SelectorState::new(
            "Dependency edge kinds",
            vec![
                SelectorOption::new(edge_flag(EDGE_KINDS[0]), "Normal dependencies", selected[0]),
                SelectorOption::new(edge_flag(EDGE_KINDS[1]), "Build dependencies", selected[1]),
                SelectorOption::new(
                    edge_flag(EDGE_KINDS[2]),
                    "Development dependencies",
                    selected[2],
                ),
                SelectorOption::new(
                    "features",
                    "Feature edges",
                    feature_edges(&self.tree_options.resolution),
                ),
            ],
        ));
        self.selector_kind = Some(SelectorKind::EdgeKinds);
        self.input_mode = InputMode::Selector;
    }

    pub(super) fn open_target_selector(&mut self) {
        let current_targets = target_selection(&self.tree_options.resolution);
        let mut candidates = target_choices().to_vec();
        for target in &self.tree_options.resolution.targets {
            if target != "all" && !candidates.contains(target) {
                candidates.push(target.clone());
            }
        }
        let mut options = Vec::with_capacity(candidates.len() + 1);
        options.push(
            SelectorOption::new(
                DEFAULT_TARGET,
                "Default target",
                current_targets
                    .iter()
                    .any(|target| target == DEFAULT_TARGET),
            )
            .exclusive(),
        );
        options.extend(candidates.iter().map(|value| {
            let option = SelectorOption::new(
                value.clone(),
                value.clone(),
                current_targets.iter().any(|target| target == value),
            );
            if value == "all" {
                option.exclusive()
            } else {
                option
            }
        }));

        self.selector = Some(
            SelectorState::new("Target triples", options)
                .filterable()
                .max_height(20),
        );
        self.selector_kind = Some(SelectorKind::Targets);
        self.input_mode = InputMode::Selector;
    }

    pub(super) fn open_feature_selector(&mut self) {
        let mut load_error = None;
        let choices = if let Some(choices) = &self.feature_choices {
            choices.clone()
        } else {
            match workspace_features(self.tree_options.manifest_path.clone()) {
                Ok(choices) => {
                    self.feature_choices = Some(choices.clone());
                    choices
                }
                Err(error) => {
                    load_error = Some(error.to_string());
                    Vec::new()
                }
            }
        };

        let resolution = &self.tree_options.resolution;
        let selected_features = resolution
            .features
            .iter()
            .flat_map(|feature| feature.split([',', ' ']))
            .filter(|feature| !feature.is_empty())
            .collect::<Vec<_>>();
        let mut options = vec![
            SelectorOption::new(
                DEFAULT_FEATURES,
                "Default features",
                !resolution.no_default_features,
            ),
            SelectorOption::new(ALL_FEATURES, "All features", resolution.all_features),
        ];
        let mut choices = choices;
        for feature in &selected_features {
            if !choices.iter().any(|choice| choice.as_str() == *feature) {
                choices.push((*feature).to_owned());
            }
        }
        choices.sort_unstable();
        choices.dedup();
        options.extend(choices.into_iter().map(|feature| {
            SelectorOption::new(
                feature.clone(),
                feature.clone(),
                selected_features.contains(&feature.as_str()),
            )
        }));

        let mut selector = SelectorState::new("Features", options)
            .filterable()
            .max_height(20);
        selector.message = load_error;
        self.selector = Some(selector);
        self.selector_kind = Some(SelectorKind::Features);
        self.input_mode = InputMode::Selector;
    }

    pub(super) fn open_package_selector(&mut self) {
        let mut load_error = None;
        let choices = if let Some(choices) = &self.package_choices {
            choices.clone()
        } else {
            match workspace_packages(self.tree_options.manifest_path.clone()) {
                Ok(choices) => {
                    self.package_choices = Some(choices.clone());
                    choices
                }
                Err(error) => {
                    load_error = Some(error.to_string());
                    Vec::new()
                }
            }
        };

        let resolution = &self.tree_options.resolution;
        let mut packages = choices;
        for package in resolution.packages.iter().chain(&resolution.exclude) {
            if !packages.contains(package) {
                packages.push(package.clone());
            }
        }
        packages.sort_unstable();
        packages.dedup();

        let mut options = vec![
            SelectorOption::new(
                WORKSPACE_PACKAGES,
                "All workspace packages",
                resolution.workspace,
            )
            .in_tab(0),
        ];
        options.extend(packages.iter().map(|package| {
            SelectorOption::new(
                format!("{INCLUDE_PACKAGE}{package}"),
                package.clone(),
                resolution.packages.contains(package),
            )
            .in_tab(0)
        }));
        options.extend(packages.iter().map(|package| {
            SelectorOption::new(
                format!("{EXCLUDE_PACKAGE}{package}"),
                package.clone(),
                resolution.exclude.contains(package),
            )
            .in_tab(1)
        }));

        let mut selector = SelectorState::new("Package selection", options)
            .filterable()
            .max_height(20)
            .tabs(vec!["Package".to_owned(), "Exclude".to_owned()]);
        selector.message = load_error;
        self.selector = Some(selector);
        self.selector_kind = Some(SelectorKind::Packages);
        self.input_mode = InputMode::Selector;
    }

    pub(super) fn open_format_selector(&mut self) {
        let fields = self.format.fields();
        let options = FORMAT_FIELDS
            .iter()
            .zip(fields)
            .map(|((value, label), checked)| SelectorOption::new(*value, *label, checked))
            .collect();
        self.selector = Some(
            SelectorState::new("Package format", options)
                .editor(self.format.raw())
                .max_height(16),
        );
        self.selector_kind = Some(SelectorKind::Format);
        self.input_mode = InputMode::Selector;
    }

    pub(super) fn handle_selector_key(&mut self, key: KeyCode) {
        let Some(selector) = self.selector.as_mut() else {
            self.input_mode = InputMode::Normal;
            return;
        };
        if selector.applying {
            if key == KeyCode::Char('q') {
                self.running = false;
            }
            return;
        }

        match key {
            KeyCode::Esc => {
                if self.selector_kind == Some(SelectorKind::Format) {
                    self.pending_format = None;
                }
                self.selector = None;
                self.selector_kind = None;
                self.input_mode = InputMode::Normal;
            }
            KeyCode::Up => selector.move_up(),
            KeyCode::Down => selector.move_down(),
            KeyCode::Left if selector.editor.is_some() => selector.move_editor_cursor_left(),
            KeyCode::Right if selector.editor.is_some() => selector.move_editor_cursor_right(),
            KeyCode::Home if selector.editor.is_some() => selector.move_editor_cursor_start(),
            KeyCode::End if selector.editor.is_some() => selector.move_editor_cursor_end(),
            KeyCode::Delete if selector.editor.is_some() => {
                selector.delete_editor_char();
                update_format_selector(selector);
            }
            KeyCode::Tab if selector.has_tabs() => selector.next_tab(),
            KeyCode::Char(' ') if self.selector_kind == Some(SelectorKind::Format) => {
                selector.toggle_selected();
                set_format_editor_from_options(selector);
            }
            KeyCode::Char(' ') if self.selector_kind == Some(SelectorKind::Packages) => {
                toggle_package_option(selector);
            }
            KeyCode::Char(' ') if selector.editor.is_none() => selector.toggle_selected(),
            KeyCode::Backspace if selector.editor.is_some() => {
                selector.backspace_editor();
                update_format_selector(selector);
            }
            KeyCode::Backspace => selector.pop_filter_char(),
            KeyCode::Char(ch) if selector.editor.is_some() => {
                selector.insert_editor_char(ch);
                update_format_selector(selector);
            }
            KeyCode::Char(ch) if selector.filter.is_some() => selector.push_filter_char(ch),
            KeyCode::Enter => self.apply_selector_selection(),
            _ => {}
        }
    }

    pub(super) fn apply_selector_selection(&mut self) {
        match self.selector_kind {
            Some(SelectorKind::EdgeKinds) => self.apply_edge_selection(),
            Some(SelectorKind::Targets) => self.apply_target_selection(),
            Some(SelectorKind::Features) => self.apply_feature_selection(),
            Some(SelectorKind::Packages) => self.apply_package_selection(),
            Some(SelectorKind::Format) => self.apply_format_selection(),
            None => {}
        }
    }

    pub(super) fn apply_format_selection(&mut self) {
        let Some(selector) = self.selector.as_ref() else {
            return;
        };
        let Some(raw_format) = selector.editor.as_deref() else {
            return;
        };
        let format = match TreeFormat::parse(raw_format) {
            Ok(format) => format,
            Err(error) => {
                if let Some(selector) = self.selector.as_mut() {
                    selector.message = Some(error.to_string());
                }
                return;
            }
        };
        if format.raw() == self.format.raw() {
            self.pending_format = None;
            self.selector = None;
            self.selector_kind = None;
            self.input_mode = InputMode::Normal;
            return;
        }

        if format_requirements_cover(
            self.format.requirements().with_metadata(),
            format.requirements(),
        ) {
            self.pending_format = None;
            self.format = format;
            self.selector = None;
            self.selector_kind = None;
            self.input_mode = InputMode::Normal;
            return;
        }

        let options = self.tree_options.clone();
        self.pending_format = Some(format.clone());
        self.resolution_generation = self.resolution_generation.wrapping_add(1);
        let request = TreeResolutionRequest {
            generation: self.resolution_generation,
            format_requirements: format.requirements().with_metadata(),
            options,
        };
        if self.resolution_tx.send(request).is_ok() {
            if let Some(selector) = self.selector.as_mut() {
                selector.applying = true;
                selector.message = None;
            }
        } else if let Some(selector) = self.selector.as_mut() {
            selector.message = Some("Could not start dependency resolution".to_owned());
        }
    }

    pub(super) fn apply_edge_selection(&mut self) {
        let Some(selector) = self.selector.as_ref() else {
            return;
        };
        let selected_values = selector.checked_values();
        let selected =
            EDGE_KINDS.map(|kind| selected_values.iter().any(|value| value == edge_flag(kind)));
        let features = selected_values.iter().any(|value| value == "features");
        if selected == edge_kinds(&self.tree_options.resolution)
            && features == feature_edges(&self.tree_options.resolution)
        {
            self.selector = None;
            self.selector_kind = None;
            self.input_mode = InputMode::Normal;
            return;
        }

        if features && self.tree_options.resolution.duplicates {
            if let Some(selector) = self.selector.as_mut() {
                selector.message =
                    Some("Feature edges cannot be combined with --duplicates".to_owned());
            }
            return;
        }

        let mut options = self.tree_options.clone();
        set_edge_options(&mut options.resolution, selected, features);
        self.resolution_generation = self.resolution_generation.wrapping_add(1);
        let request = TreeResolutionRequest {
            generation: self.resolution_generation,
            format_requirements: self.format.requirements().with_metadata(),
            options,
        };

        if self.resolution_tx.send(request).is_ok() {
            if let Some(selector) = self.selector.as_mut() {
                selector.applying = true;
                selector.message = None;
            }
        } else if let Some(selector) = self.selector.as_mut() {
            selector.message = Some("Could not start dependency resolution".to_owned());
        }
    }

    pub(super) fn apply_target_selection(&mut self) {
        let Some(selector) = self.selector.as_ref() else {
            return;
        };
        let selected = selector.checked_values();
        let selected_targets = if selected.iter().any(|target| target == "all") {
            vec!["all".to_owned()]
        } else if selected.is_empty() || selected.iter().all(|target| target == DEFAULT_TARGET) {
            vec![DEFAULT_TARGET.to_owned()]
        } else {
            selected
                .into_iter()
                .filter(|target| target != DEFAULT_TARGET)
                .collect()
        };
        if selected_targets == target_selection(&self.tree_options.resolution) {
            self.selector = None;
            self.selector_kind = None;
            self.input_mode = InputMode::Normal;
            return;
        }

        let mut options = self.tree_options.clone();
        options.resolution.targets = if selected_targets == [DEFAULT_TARGET] {
            Vec::new()
        } else {
            selected_targets
        };
        options.resolution.all_targets = false;
        self.resolution_generation = self.resolution_generation.wrapping_add(1);
        let request = TreeResolutionRequest {
            generation: self.resolution_generation,
            format_requirements: self.format.requirements().with_metadata(),
            options,
        };

        if self.resolution_tx.send(request).is_ok() {
            if let Some(selector) = self.selector.as_mut() {
                selector.applying = true;
                selector.message = None;
            }
        } else if let Some(selector) = self.selector.as_mut() {
            selector.message = Some("Could not start dependency resolution".to_owned());
        }
    }

    pub(super) fn apply_feature_selection(&mut self) {
        let Some(selector) = self.selector.as_ref() else {
            return;
        };
        let selected = selector.checked_values();
        let features = selected
            .iter()
            .filter(|feature| {
                feature.as_str() != DEFAULT_FEATURES && feature.as_str() != ALL_FEATURES
            })
            .cloned()
            .collect::<Vec<_>>();
        let all_features = selected
            .iter()
            .any(|feature| feature.as_str() == ALL_FEATURES);
        let no_default_features = !selected
            .iter()
            .any(|feature| feature.as_str() == DEFAULT_FEATURES);
        let current = &self.tree_options.resolution;
        let mut current_features = current.features.clone();
        current_features.sort_unstable();
        if features == current_features
            && all_features == current.all_features
            && no_default_features == current.no_default_features
        {
            self.selector = None;
            self.selector_kind = None;
            self.input_mode = InputMode::Normal;
            return;
        }

        let mut options = self.tree_options.clone();
        options.resolution.features = features;
        options.resolution.all_features = all_features;
        options.resolution.no_default_features = no_default_features;
        self.resolution_generation = self.resolution_generation.wrapping_add(1);
        let request = TreeResolutionRequest {
            generation: self.resolution_generation,
            format_requirements: self.format.requirements().with_metadata(),
            options,
        };

        if self.resolution_tx.send(request).is_ok() {
            if let Some(selector) = self.selector.as_mut() {
                selector.applying = true;
                selector.message = None;
            }
        } else if let Some(selector) = self.selector.as_mut() {
            selector.message = Some("Could not start dependency resolution".to_owned());
        }
    }

    pub(super) fn apply_package_selection(&mut self) {
        let Some(selector) = self.selector.as_ref() else {
            return;
        };
        let selected = selector.checked_values();
        let workspace = selected.iter().any(|value| value == WORKSPACE_PACKAGES);
        let packages = selected
            .iter()
            .filter_map(|value| value.strip_prefix(INCLUDE_PACKAGE))
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let exclude = selected
            .iter()
            .filter_map(|value| value.strip_prefix(EXCLUDE_PACKAGE))
            .map(str::to_owned)
            .collect::<Vec<_>>();

        let current = &self.tree_options.resolution;
        let mut current_packages = current.packages.clone();
        current_packages.sort_unstable();
        let mut current_exclude = current.exclude.clone();
        current_exclude.sort_unstable();
        if workspace == current.workspace
            && packages == current_packages
            && exclude == current_exclude
        {
            self.selector = None;
            self.selector_kind = None;
            self.input_mode = InputMode::Normal;
            return;
        }

        let mut options = self.tree_options.clone();
        options.resolution.workspace = workspace;
        options.resolution.packages = packages;
        options.resolution.exclude = exclude;
        self.resolution_generation = self.resolution_generation.wrapping_add(1);
        let request = TreeResolutionRequest {
            generation: self.resolution_generation,
            format_requirements: self.format.requirements().with_metadata(),
            options,
        };

        if self.resolution_tx.send(request).is_ok() {
            if let Some(selector) = self.selector.as_mut() {
                selector.applying = true;
                selector.message = None;
            }
        } else if let Some(selector) = self.selector.as_mut() {
            selector.message = Some("Could not start dependency resolution".to_owned());
        }
    }
}

const EDGE_KINDS: [DepKind; 3] = [DepKind::Normal, DepKind::Build, DepKind::Development];

const DEFAULT_TARGET: &str = "__cargo_tree_tui_default_target";
const DEFAULT_FEATURES: &str = "__cargo_tree_tui_default_features";
const ALL_FEATURES: &str = "__cargo_tree_tui_all_features";
const WORKSPACE_PACKAGES: &str = "__cargo_tree_tui_workspace_packages";
const INCLUDE_PACKAGE: &str = "__cargo_tree_tui_include_package:";
const EXCLUDE_PACKAGE: &str = "__cargo_tree_tui_exclude_package:";
const FORMAT_FIELDS: [(&str, &str); 5] = [
    ("{p}", "Package (name, version, source)"),
    ("{l}", "License"),
    ("{r}", "Repository"),
    ("{f}", "Enabled features"),
    ("{lib}", "Library name"),
];

fn set_format_editor_from_options(selector: &mut SelectorState) {
    let format = selector
        .options
        .iter()
        .filter(|option| option.checked)
        .map(|option| option.value.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    selector.set_editor(format);
    selector.message = None;
}

fn update_format_selector(selector: &mut SelectorState) {
    let Some(editor) = selector.editor.as_deref() else {
        return;
    };
    match TreeFormat::parse(editor) {
        Ok(format) => {
            for (option, checked) in selector.options.iter_mut().zip(format.fields()) {
                option.checked = checked;
            }
            selector.message = None;
        }
        Err(error) => selector.message = Some(error.to_string()),
    }
}

fn format_requirements_cover(
    loaded: PackageFormatRequirements,
    requested: PackageFormatRequirements,
) -> bool {
    (!requested.metadata || loaded.metadata)
        && (!requested.license || loaded.license)
        && (!requested.repository || loaded.repository)
        && (!requested.features || loaded.features)
        && (!requested.library || loaded.library)
        && (!requested.source || loaded.source)
}

fn toggle_package_option(selector: &mut SelectorState) {
    let Some(&option_index) = selector.visible_options().get(selector.selected) else {
        return;
    };
    let value = selector.options[option_index].value.clone();
    selector.toggle_selected();

    if !selector.options[option_index].checked {
        if value == WORKSPACE_PACKAGES {
            for option in &mut selector.options {
                if option.value.starts_with(EXCLUDE_PACKAGE) {
                    option.checked = false;
                }
            }
        }
        return;
    }

    match value.as_str() {
        WORKSPACE_PACKAGES => {
            for option in &mut selector.options {
                if option.value.starts_with(INCLUDE_PACKAGE) {
                    option.checked = false;
                }
            }
        }
        value if value.starts_with(INCLUDE_PACKAGE) => {
            for option in &mut selector.options {
                if option.value == WORKSPACE_PACKAGES || option.value.starts_with(EXCLUDE_PACKAGE) {
                    option.checked = false;
                }
            }
        }
        value if value.starts_with(EXCLUDE_PACKAGE) => {
            for option in &mut selector.options {
                if option.value == WORKSPACE_PACKAGES {
                    option.checked = true;
                } else if option.value.starts_with(INCLUDE_PACKAGE) {
                    option.checked = false;
                }
            }
        }
        _ => {}
    }
}

fn target_choices() -> &'static [String] {
    static TARGET_CHOICES: OnceLock<Vec<String>> = OnceLock::new();
    TARGET_CHOICES
        .get_or_init(|| {
            crate::util::command_prelude::get_target_tuples_with_all()
                .into_iter()
                .map(|candidate| candidate.get_value().to_string_lossy().into_owned())
                .collect()
        })
        .as_slice()
}

fn target_selection(options: &ResolutionOptions) -> Vec<String> {
    if options.all_targets || options.targets.iter().any(|target| target == "all") {
        vec!["all".to_owned()]
    } else if options.targets.is_empty() {
        vec![DEFAULT_TARGET.to_owned()]
    } else {
        options.targets.clone()
    }
}

fn edge_flag(kind: DepKind) -> &'static str {
    match kind {
        DepKind::Normal => "normal",
        DepKind::Build => "build",
        DepKind::Development => "dev",
    }
}

fn excluded_edge_flag(kind: DepKind) -> &'static str {
    match kind {
        DepKind::Normal => "no-normal",
        DepKind::Build => "no-build",
        DepKind::Development => "no-dev",
    }
}

fn edge_kinds(options: &ResolutionOptions) -> [bool; 3] {
    let edges = &options.edges;
    let has_all = edges.iter().any(|kind| kind == "all");
    let has_exclusions = EDGE_KINDS
        .iter()
        .any(|kind| edges.iter().any(|edge| edge == excluded_edge_flag(*kind)));
    let has_inclusions = EDGE_KINDS
        .iter()
        .any(|kind| edges.iter().any(|edge| edge == edge_flag(*kind)));

    let mut selected = if has_all || (!has_exclusions && !has_inclusions) {
        [true; 3]
    } else if has_exclusions {
        std::array::from_fn(|index| {
            !edges
                .iter()
                .any(|edge| edge == excluded_edge_flag(EDGE_KINDS[index]))
        })
    } else {
        std::array::from_fn(|index| {
            edges
                .iter()
                .any(|edge| edge == edge_flag(EDGE_KINDS[index]))
        })
    };

    if options.no_dev_dependencies {
        selected[2] = false;
    }
    selected
}

fn feature_edges(options: &ResolutionOptions) -> bool {
    options
        .edges
        .iter()
        .any(|kind| matches!(kind.as_str(), "features" | "all"))
}

fn set_edge_options(options: &mut ResolutionOptions, selected: [bool; 3], features: bool) {
    options.edges.retain(|kind| {
        !matches!(kind.as_str(), "all" | "features")
            && !EDGE_KINDS.iter().any(|edge| kind == edge_flag(*edge))
            && !EDGE_KINDS
                .iter()
                .any(|edge| kind == excluded_edge_flag(*edge))
    });
    options.no_dev_dependencies = false;

    if selected == [true; 3] && features {
        options.edges.push("all".to_owned());
        return;
    }
    if features {
        options.edges.push("features".to_owned());
    }
    if selected == [true; 3] {
        options.edges.extend(
            EDGE_KINDS
                .into_iter()
                .map(|kind| edge_flag(kind).to_owned()),
        );
        return;
    }

    let use_exclusions = selected == [false; 3];
    options.edges.extend(
        EDGE_KINDS
            .into_iter()
            .zip(selected)
            .filter_map(|(kind, is_selected)| {
                let include_kind = if use_exclusions {
                    !is_selected
                } else {
                    is_selected
                };
                include_kind.then(|| {
                    if use_exclusions {
                        excluded_edge_flag(kind)
                    } else {
                        edge_flag(kind)
                    }
                    .to_owned()
                })
            }),
    );
}
