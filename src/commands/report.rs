use std::cmp::Ordering;
use std::fmt::Debug;
use std::io;
use std::path::{Path, PathBuf};

use rayon::prelude::*;

use thiserror::Error;

use crate::colors::*;

use crate::cli::create_clickable_link;
use crate::config::root_module::RootModuleTreatment;
use crate::config::ProjectConfig;
use crate::filesystem::{
    file_to_module_path, validate_project_modules, walk_pyfiles, FileSystemError,
};
use crate::interrupt::check_interrupt;
use crate::modules::{build_module_tree, error::ModuleTreeError};
use crate::processors::imports::{get_project_imports, ImportParseError, NormalizedImport};

struct Dependency {
    file_path: PathBuf,
    absolute_path: PathBuf,
    import: NormalizedImport,
    source_module: String,
    target_module: String,
}

#[derive(Error, Debug)]
pub enum ReportCreationError {
    #[error("I/O failure during report generation:\n{0}")]
    Io(#[from] io::Error),
    #[error("Filesystem error: {0}")]
    Filesystem(#[from] FileSystemError),
    #[error("Import parsing error: {0}")]
    ImportParse(#[from] ImportParseError),
    #[error("Nothing to report when skipping dependencies and usages.")]
    NothingToReport,
    #[error("Module tree build error: {0}")]
    ModuleTree(#[from] ModuleTreeError),
    #[error("Operation interrupted")]
    Interrupted,
}

pub type Result<T> = std::result::Result<T, ReportCreationError>;

// less code than implementing/deriving all necessary traits for Ord
fn compare_dependencies(left: &Dependency, right: &Dependency) -> Ordering {
    let path_cmp = left.file_path.cmp(&right.file_path);
    if path_cmp == Ordering::Equal {
        return left.import.line_no.cmp(&right.import.line_no);
    }
    path_cmp
}

struct DependencyReport {
    path: String,
    pub dependencies: Vec<Dependency>,
    pub usages: Vec<Dependency>,
    pub warnings: Vec<String>,
}

impl DependencyReport {
    fn new(path: String) -> Self {
        DependencyReport {
            path,
            dependencies: vec![],
            usages: vec![],
            warnings: vec![],
        }
    }

    fn render_dependency(&self, dependency: &Dependency) -> String {
        let clickable_link = create_clickable_link(
            &dependency.file_path,
            &dependency.absolute_path,
            &dependency.import.line_no,
        );
        format!(
            "{green}{clickable_link}{end_color}: {cyan}Import '{import_mod_path}'{end_color}",
            green = BColors::OKGREEN,
            clickable_link = clickable_link,
            end_color = BColors::ENDC,
            cyan = BColors::OKCYAN,
            import_mod_path = dependency.import.module_path
        )
    }

    fn render_to_string(
        &mut self,
        skip_dependencies: bool,
        skip_usages: bool,
        raw: bool,
    ) -> String {
        if raw {
            let mut lines = Vec::new();

            if !skip_dependencies && !self.dependencies.is_empty() {
                lines.push("# Module Dependencies".to_string());
                let mut module_paths: Vec<_> = self
                    .dependencies
                    .iter()
                    .map(|dep| dep.target_module.clone())
                    .collect();
                module_paths.sort();
                module_paths.dedup();
                lines.extend(module_paths);
            }

            if !skip_usages && !self.usages.is_empty() {
                lines.push("# Module Usages".to_string());
                let mut using_modules: Vec<_> = self
                    .usages
                    .iter()
                    .map(|usage| usage.source_module.clone())
                    .collect();
                using_modules.sort();
                using_modules.dedup();
                lines.extend(using_modules);
            }

            return lines.join("\n");
        }

        let title = format!("Dependency Report for '{path}'", path = self.path.as_str());
        let mut result = format!(
            "[ {title} ]\n\
            -------------------------------\n",
            title = title,
        );

        if !skip_dependencies {
            let deps_title = format!("Dependencies of '{path}'", path = self.path.as_str());
            self.dependencies.sort_by(compare_dependencies);
            let deps_display: String = match self.dependencies.len() {
                0 => format!(
                    "{cyan}No dependencies found.{end_color}",
                    cyan = BColors::WARNING,
                    end_color = BColors::ENDC
                ),
                _ => self
                    .dependencies
                    .iter()
                    .map(|dep| self.render_dependency(dep))
                    .collect::<Vec<String>>()
                    .join("\n")
                    .to_string(),
            };
            result.push_str(&format!(
                "[ {deps_title} ]\n\
                {deps}\n\
                -------------------------------\n",
                deps_title = deps_title,
                deps = deps_display,
            ));
        }

        if !skip_usages {
            let usages_title = format!("Usages of '{path}'", path = self.path.as_str());
            self.usages.sort_by(compare_dependencies);
            let usages_display: String = match self.usages.len() {
                0 => format!(
                    "{cyan}No usages found.{end_color}",
                    cyan = BColors::WARNING,
                    end_color = BColors::ENDC
                ),
                _ => self
                    .usages
                    .iter()
                    .map(|dep| self.render_dependency(dep))
                    .collect::<Vec<String>>()
                    .join("\n")
                    .to_string(),
            };
            result.push_str(&format!(
                "[ {usages_title} ]\n\
                {usages}\n\
                -------------------------------\n",
                usages_title = usages_title,
                usages = usages_display,
            ));
        }

        if !self.warnings.is_empty() {
            result.push_str(&format!(
                "[ Warnings ]\n\
                {warning_color}{warnings}{end_color}",
                warning_color = BColors::WARNING,
                end_color = BColors::ENDC,
                warnings = self.warnings.join("\n")
            ));
        }

        result
    }
}

fn is_module_prefix(prefix: &str, full_path: &str) -> bool {
    if !full_path.starts_with(prefix) {
        return false;
    }
    full_path.len() == prefix.len() || full_path[prefix.len()..].starts_with('.')
}

pub fn create_dependency_report(
    project_root: &Path,
    project_config: &ProjectConfig,
    path: &PathBuf,
    include_dependency_modules: Option<Vec<String>>,
    include_usage_modules: Option<Vec<String>>,
    skip_dependencies: bool,
    skip_usages: bool,
    raw: bool,
) -> Result<String> {
    if skip_dependencies && skip_usages {
        return Err(ReportCreationError::NothingToReport);
    }

    let source_roots = project_config.prepend_roots(project_root);
    let (valid_modules, _) = validate_project_modules(
        &source_roots,
        project_config.all_modules().cloned().collect(),
    );

    check_interrupt().map_err(|_| ReportCreationError::Interrupted)?;

    let module_tree = build_module_tree(
        &source_roots,
        &valid_modules,
        false,                      // skip circular dependency check in report
        RootModuleTreatment::Allow, // skip root module check in report
    )?;

    let absolute_path = project_root.join(path);
    let module_path = file_to_module_path(&source_roots, &absolute_path)?;
    let target_module = module_tree.find_nearest(&module_path).ok_or_else(|| {
        ReportCreationError::ModuleTree(ModuleTreeError::ModuleNotFound(module_path.clone()))
    })?;

    let mut report = DependencyReport::new(path.display().to_string());

    for source_root in &source_roots {
        check_interrupt().map_err(|_| ReportCreationError::Interrupted)?;

        let source_root_results: Vec<_> = walk_pyfiles(&source_root.display().to_string())
            .par_bridge()
            .filter_map(|pyfile| {
                if check_interrupt().is_err() {
                    return None;
                }

                let absolute_pyfile = source_root.join(&pyfile);
                let file_module_path = match file_to_module_path(&source_roots, &absolute_pyfile) {
                    Ok(path) => path,
                    Err(_) => return None,
                };
                let file_module = module_tree.find_nearest(&file_module_path);

                match get_project_imports(
                    &source_roots,
                    &absolute_pyfile,
                    project_config.ignore_type_checking_imports,
                    project_config.include_string_imports,
                ) {
                    Ok(project_imports) => {
                        let is_in_target_path = is_module_prefix(&module_path, &file_module_path);
                        let mut dependencies = Vec::new();
                        let mut usages = Vec::new();

                        if is_in_target_path && !skip_dependencies {
                            // Add dependencies
                            dependencies.extend(
                                project_imports
                                    .imports
                                    .iter()
                                    .filter_map(|import| {
                                        if let Some(import_module) =
                                            module_tree.find_nearest(&import.module_path)
                                        {
                                            if import_module == target_module {
                                                return None;
                                            }
                                            include_dependency_modules.as_ref().map_or(
                                                Some((import.clone(), import_module.clone())),
                                                |included_modules| {
                                                    if included_modules
                                                        .contains(&import_module.full_path)
                                                    {
                                                        Some((
                                                            import.clone(),
                                                            import_module.clone(),
                                                        ))
                                                    } else {
                                                        None
                                                    }
                                                },
                                            )
                                        } else {
                                            None
                                        }
                                    })
                                    .map(|(import, import_module)| Dependency {
                                        file_path: pyfile.clone(),
                                        absolute_path: absolute_pyfile.clone(),
                                        import,
                                        source_module: target_module.full_path.clone(),
                                        target_module: import_module.full_path.clone(),
                                    }),
                            );
                        } else if !is_in_target_path && !skip_usages {
                            // Add usages
                            usages.extend(
                                project_imports
                                    .imports
                                    .iter()
                                    .filter(|import| {
                                        if !is_module_prefix(&module_path, &import.module_path) {
                                            return false;
                                        }
                                        file_module.as_ref().map_or(false, |m| {
                                            include_usage_modules.as_ref().map_or(
                                                true,
                                                |included_modules| {
                                                    included_modules.contains(&m.full_path)
                                                },
                                            )
                                        })
                                    })
                                    .map(|import| Dependency {
                                        file_path: pyfile.clone(),
                                        absolute_path: absolute_pyfile.clone(),
                                        import: import.clone(),
                                        source_module: file_module
                                            .as_ref()
                                            .map_or(String::new(), |m| m.full_path.clone()),
                                        target_module: target_module.full_path.clone(),
                                    }),
                            );
                        }

                        Some((dependencies, usages, None))
                    }
                    Err(err) => Some((Vec::new(), Vec::new(), Some(err.to_string()))),
                }
            })
            .collect();

        check_interrupt().map_err(|_| ReportCreationError::Interrupted)?;

        // Combine results
        for (dependencies, usages, warning) in source_root_results {
            report.dependencies.extend(dependencies);
            report.usages.extend(usages);
            if let Some(warning) = warning {
                report.warnings.push(warning);
            }
        }
    }

    Ok(report.render_to_string(skip_dependencies, skip_usages, raw))
}
