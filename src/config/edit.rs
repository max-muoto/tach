use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub enum ConfigEdit {
    CreateModule { path: String },
    DeleteModule { path: String },
    MarkModuleAsUtility { path: String },
    UnmarkModuleAsUtility { path: String },
    AddDependency { path: String, dependency: String },
    RemoveDependency { path: String, dependency: String },
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum EditError {
    #[error("Edit not applicable")]
    NotApplicable,
    #[error("Module not found")]
    ModuleNotFound,
    #[error("Module already exists")]
    ModuleAlreadyExists,
    #[error("Failed to parse config")]
    ParsingFailed,
    #[error("Failed to write to disk")]
    DiskWriteFailed,
    #[error("Config file does not exist")]
    ConfigDoesNotExist,
}

pub trait ConfigEditor {
    fn enqueue_edit(&mut self, edit: &ConfigEdit) -> Result<(), EditError>;
    fn apply_edits(&mut self) -> Result<(), EditError>;
}
