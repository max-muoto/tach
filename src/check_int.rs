use std::{
    collections::HashSet,
    error::Error,
    path::{Path, PathBuf},
    rc::Rc,
};

use pyo3::{pyclass, pymethods};
use thiserror::Error;

use crate::{
    core::{
        config::parse_project_config,
        module::{ModuleNode, ModuleTree},
    },
    exclusion::{is_path_excluded, set_excluded_paths},
    filesystem as fs,
    imports::{get_project_imports, ImportParseError},
    parsing::module::build_module_tree,
};

#[derive(Error, Debug, Clone)]
#[pyclass(module = "tach.extension")]

pub enum ImportCheckError {
    #[error("Module containing '{file_mod_path}' not found in project.")]
    ModuleNotFound { file_mod_path: String },

    #[error("Module '{import_nearest_module_path}' is in strict mode. Only imports from the public interface of this module are allowed. The import '{import_mod_path}' (in module '{file_nearest_module_path}') is not included in __all__.")]
    StrictModeImport {
        import_mod_path: String,
        import_nearest_module_path: String,
        file_nearest_module_path: String,
    },

    #[error("Could not find module configuration.")]
    ModuleConfigNotFound(),

    #[error("Cannot import '{import_mod_path}'. Module '{source_module}' cannot depend on '{invalid_module}'.")]
    InvalidImport {
        import_mod_path: String,
        source_module: String,
        invalid_module: String,
    },

    #[error("Import '{import_mod_path}' is deprecated. Module '{source_module}' should not depend on '{invalid_module}'.")]
    DeprecatedImport {
        import_mod_path: String,
        source_module: String,
        invalid_module: String,
    },
}

#[pymethods]
impl ImportCheckError {
    pub fn is_dependency_error(&self) -> bool {
        matches!(
            self,
            Self::InvalidImport { .. } | Self::DeprecatedImport { .. }
        )
    }

    pub fn is_deprecated(&self) -> bool {
        matches!(self, Self::DeprecatedImport { .. })
    }

    pub fn to_pystring(&self) -> String {
        self.to_string()
    }
}

fn is_top_level_module_import(mod_path: &str, module: &ModuleNode) -> bool {
    mod_path == module.full_path
}

fn import_matches_interface_members(mod_path: &str, module: &ModuleNode) -> bool {
    let mod_path_segments: Vec<&str> = mod_path.rsplitn(2, '.').collect();

    if mod_path_segments.len() == 1 {
        // If there's no '.' in the path, compare the whole path with the module's full path.
        mod_path_segments[0] == module.full_path
    } else {
        // If there's a '.', split into package path and member name.
        let mod_pkg_path = mod_path_segments[1];
        let mod_member_name = mod_path_segments[0];

        mod_pkg_path == module.full_path
            && module
                .interface_members
                .contains(&mod_member_name.to_string())
    }
}

fn check_import(
    module_tree: &ModuleTree,
    import_mod_path: &str,
    file_mod_path: &str,
    file_nearest_module: Option<Rc<ModuleNode>>,
) -> Result<(), ImportCheckError> {
    let import_nearest_module = match module_tree.find_nearest(import_mod_path) {
        Some(module) => module,
        // This should not be none since we intend to filter out any external imports,
        // but we should allow external imports if they have made it here.
        None => return Ok(()),
    };

    let file_nearest_module = file_nearest_module
        // Lookup file_mod_path if module not given
        .or_else(|| module_tree.find_nearest(file_mod_path))
        // If module not found, we should fail since the implication is that
        // an external module is importing directly from our project
        .ok_or(ImportCheckError::ModuleNotFound {
            file_mod_path: file_mod_path.to_string(),
        })?;

    if import_nearest_module == file_nearest_module {
        // Imports within the same module are always allowed
        return Ok(());
    }

    if let Some(config) = &import_nearest_module.config {
        if config.strict
            && !is_top_level_module_import(import_mod_path, &import_nearest_module)
            && !import_matches_interface_members(import_mod_path, &import_nearest_module)
        {
            // In strict mode, import must be of the module itself or one of the
            // interface members (defined in __all__)
            return Err(ImportCheckError::StrictModeImport {
                import_mod_path: import_mod_path.to_string(),
                import_nearest_module_path: import_nearest_module.full_path.to_string(),
                file_nearest_module_path: file_nearest_module.full_path.to_string(),
            });
        }
    }

    let file_config = file_nearest_module
        .config
        .as_ref()
        .ok_or(ImportCheckError::ModuleConfigNotFound())?;
    let file_nearest_module_path = &file_config.path;
    let import_nearest_module_path = &import_nearest_module
        .config
        .as_ref()
        .ok_or(ImportCheckError::ModuleConfigNotFound())?
        .path;

    // The import must be explicitly allowed in the file's config
    let allowed_dependencies: HashSet<_> = file_config
        .depends_on
        .iter()
        .filter(|dep| !dep.deprecated)
        .map(|dep| &dep.path)
        .collect();

    if allowed_dependencies.contains(import_nearest_module_path) {
        // he import matches at least one expected dependency
        return Ok(());
    }

    let deprecated_dependencies: HashSet<_> = file_config
        .depends_on
        .iter()
        .filter(|dep| dep.deprecated)
        .map(|dep| &dep.path)
        .collect();

    if deprecated_dependencies.contains(import_nearest_module_path) {
        // Dependency exists but is deprecated
        return Err(ImportCheckError::DeprecatedImport {
            import_mod_path: import_mod_path.to_string(),
            source_module: file_nearest_module_path.to_string(),
            invalid_module: import_nearest_module_path.to_string(),
        });
    }

    // This means the import is not declared as a dependency of the file
    Err(ImportCheckError::InvalidImport {
        import_mod_path: import_mod_path.to_string(),
        source_module: file_nearest_module_path.to_string(),
        invalid_module: import_nearest_module_path.to_string(),
    })
}

#[derive(Debug, Clone)]
#[pyclass(get_all, module = "tach.extension")]

pub struct BoundaryError {
    pub file_path: PathBuf,
    pub line_number: usize,
    pub import_mod_path: String,
    pub error_info: ImportCheckError,
}

#[derive(Debug)]
#[pyclass(get_all, module = "tach.extension")]
pub struct CheckDiagnostics {
    pub errors: Vec<BoundaryError>,
    pub deprecated_warnings: Vec<BoundaryError>,
    pub warnings: Vec<String>,
}

pub fn check(
    project_root: String,
    project_config_path: String,
    exclude_paths: Vec<String>,
) -> Result<CheckDiagnostics, Box<dyn Error>> {
    let project_root = Path::new(&project_root);

    let exclude_paths = exclude_paths.iter().map(PathBuf::from).collect::<Vec<_>>();
    if !project_root.is_dir() {
        eprintln!(
            "The path {} is not a valid directory.",
            project_root.display()
        );
    }
    let project_config = parse_project_config(project_config_path)?;
    let source_roots: Vec<PathBuf> = project_config
        .source_roots
        .into_iter()
        .map(|r| project_root.join(r))
        .collect();
    let (valid_modules, invalid_modules) =
        fs::validate_project_modules(&source_roots, project_config.modules);

    let mut found_at_least_one_project_import = false;
    let mut boundary_errors = Vec::new();
    let mut boundary_warnings = Vec::new();
    let mut warnings = Vec::new();

    for module in &invalid_modules {
        warnings.push(format!(
            "Module '{}' not found. It will be ignored.",
            module.path
        ));
    }

    let module_tree = build_module_tree(
        &source_roots,
        valid_modules,
        project_config.forbid_circular_dependencies,
    )?;

    set_excluded_paths(
        project_root,
        &exclude_paths,
        project_config.use_regex_matching,
    )?;

    for source_root in &source_roots {
        for file_path in fs::walk_pyfiles(&source_root.display().to_string()) {
            let abs_file_path = &source_root.join(&file_path);
            if is_path_excluded(&abs_file_path.display().to_string())? {
                continue;
            }
            let mod_path = fs::file_to_module_path(&source_roots, abs_file_path)?;
            let Some(nearest_module) = module_tree.find_nearest(&mod_path) else {
                continue;
            };
            let project_imports = match get_project_imports(
                &source_roots,
                abs_file_path,
                project_config.ignore_type_checking_imports,
            ) {
                Ok(v) => v,
                Err(ImportParseError::Parsing { .. }) => {
                    eprintln!("Skipping '{}' due to a syntax error.", file_path.display());
                    continue;
                }
                Err(ImportParseError::Filesystem(_)) => {
                    eprintln!("Skipping '{}' due to an I/O error.", file_path.display());
                    continue;
                }
                Err(ImportParseError::Exclusion(_)) => {
                    eprintln!(
                        "Skipping '{}'. Filed to check if the path is excluded.",
                        file_path.display()
                    );
                    continue;
                }
            };

            for import in project_imports {
                found_at_least_one_project_import = true;
                let Err(error_info) = check_import(
                    &module_tree,
                    &import.module_path,
                    &mod_path,
                    Some(Rc::clone(&nearest_module)),
                ) else {
                    continue;
                };
                let boundary_error = BoundaryError {
                    file_path: file_path.clone(),
                    line_number: import.line_no,
                    import_mod_path: import.module_path.to_string(),
                    error_info,
                };
                if boundary_error.error_info.is_deprecated() {
                    boundary_warnings.push(boundary_error);
                } else {
                    boundary_errors.push(boundary_error);
                }
            }
        }
    }

    if !found_at_least_one_project_import {
        warnings.push(
            "WARNING: No first-party imports were found. You may need to use 'tach mod' to update your Python source roots. Docs: https://docs.gauge.sh/usage/configuration#source-roots"
                .to_string(),
        );
    }

    Ok(CheckDiagnostics {
        errors: boundary_errors,
        deprecated_warnings: boundary_warnings,
        warnings,
    })
}
