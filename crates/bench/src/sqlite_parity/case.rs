use std::fmt;
use std::str::FromStr;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Priority {
    P0,
    P1,
    P2,
    P3,
    P4,
}

impl Priority {
    pub fn is_quarantine(self) -> bool {
        matches!(self, Self::P4)
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::P0 => "P0",
            Self::P1 => "P1",
            Self::P2 => "P2",
            Self::P3 => "P3",
            Self::P4 => "P4",
        })
    }
}

impl FromStr for Priority {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.trim().to_ascii_uppercase().as_str() {
            "P0" => Ok(Self::P0),
            "P1" => Ok(Self::P1),
            "P2" => Ok(Self::P2),
            "P3" => Ok(Self::P3),
            "P4" => Ok(Self::P4),
            other => bail!("unknown priority `{other}`"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Profile {
    Memory,
    Tempfile,
    Catalog,
    ExternalApp,
    SideEffect,
}

impl fmt::Display for Profile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Memory => "memory",
            Self::Tempfile => "tempfile",
            Self::Catalog => "catalog",
            Self::ExternalApp => "external_app",
            Self::SideEffect => "side_effect",
        })
    }
}

impl FromStr for Profile {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "memory" | "mem" => Ok(Self::Memory),
            "tempfile" | "file" | "tmp" => Ok(Self::Tempfile),
            "catalog" => Ok(Self::Catalog),
            "external_app" | "external-app" => Ok(Self::ExternalApp),
            "side_effect" | "side-effect" => Ok(Self::SideEffect),
            other => bail!("unknown profile `{other}`"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Case {
    pub id: usize,
    pub folder: String,
    pub name: String,
    pub category: String,
    pub priority: Priority,
    pub profile: Profile,
    pub kind: String,
    pub description: String,
    pub status: String,
    pub db: String,
    pub args: Vec<String>,
    pub stdin: String,
    pub expected_exit: i32,
    pub compare_stdout: bool,
    pub expected_stdout: Option<String>,
    pub expected_stdout_contains: Vec<String>,
    pub expected_stderr_contains: Vec<String>,
    pub expected_combined_contains: Vec<String>,
    pub files: Vec<(String, String)>,
    pub script: Option<String>,
    pub notes: String,
}

impl Case {
    pub fn display_id(&self) -> String {
        format!("{:05}", self.id)
    }

    pub fn case_file_name(&self) -> String {
        format!("{}.rs", self.folder)
    }
}
