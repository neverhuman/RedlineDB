use std::str::FromStr;

use anyhow::{Result, bail};

use super::case::{Case, Priority, Profile};

#[derive(Debug, Clone)]
pub struct Selection {
    pub priorities: Vec<Priority>,
    pub profiles: Vec<Profile>,
    pub include_quarantine: bool,
}

impl Default for Selection {
    fn default() -> Self {
        Self {
            priorities: vec![Priority::P0, Priority::P1, Priority::P2],
            profiles: vec![Profile::Memory, Profile::Tempfile],
            include_quarantine: false,
        }
    }
}

impl Selection {
    pub fn from_cli(
        priorities: Option<&str>,
        profiles: Option<&str>,
        include_quarantine: bool,
    ) -> Result<Self> {
        let mut selection = Self::default();
        if let Some(priorities) = priorities {
            selection.priorities = parse_csv(priorities)?;
        }
        if let Some(profiles) = profiles {
            selection.profiles = parse_csv(profiles)?;
        }
        selection.include_quarantine = include_quarantine;
        Ok(selection)
    }

    pub fn matches(&self, case: &Case) -> bool {
        self.priorities.contains(&case.priority)
            && self.profiles.contains(&case.profile)
            && (self.include_quarantine || !case.priority.is_quarantine())
            && case.status == "active"
    }
}

fn parse_csv<T>(value: &str) -> Result<Vec<T>>
where
    T: FromStr<Err = anyhow::Error> + Eq,
{
    let parsed = value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(T::from_str)
        .collect::<Result<Vec<_>>>()?;
    if parsed.is_empty() {
        bail!("empty comma-separated selector");
    }
    let mut deduped = Vec::new();
    for item in parsed {
        if !deduped.contains(&item) {
            deduped.push(item);
        }
    }
    Ok(deduped)
}
