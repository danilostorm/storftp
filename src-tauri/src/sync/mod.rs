use crate::error::Result;
use crate::models::{FileEntry, ProviderRef};
use crate::providers::ProviderFactory;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CompareState { Equal, OnlyLeft, OnlyRight, Different, Conflict }

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompareEntry { pub name: String, pub state: CompareState, pub left: Option<FileEntry>, pub right: Option<FileEntry> }

pub fn compare(factory: &ProviderFactory, left: &ProviderRef, left_path: &str, right: &ProviderRef, right_path: &str) -> Result<Vec<CompareEntry>> {
    let left_items = factory.build(left)?.list(left_path)?;
    let right_items = factory.build(right)?.list(right_path)?;
    let left_map: BTreeMap<String, FileEntry> = left_items.into_iter().map(|e| (e.name.clone(), e)).collect();
    let right_map: BTreeMap<String, FileEntry> = right_items.into_iter().map(|e| (e.name.clone(), e)).collect();
    let names = left_map.keys().chain(right_map.keys()).cloned().collect::<BTreeSet<_>>();
    Ok(names.into_iter().map(|name| {
        let left = left_map.get(&name).cloned(); let right = right_map.get(&name).cloned();
        let state = match (&left, &right) {
            (Some(a), Some(b)) if a.is_dir != b.is_dir => CompareState::Conflict,
            (Some(a), Some(b)) if a.is_dir || (a.size == b.size && a.modified_at == b.modified_at) => CompareState::Equal,
            (Some(_), Some(_)) => CompareState::Different,
            (Some(_), None) => CompareState::OnlyLeft,
            (None, Some(_)) => CompareState::OnlyRight,
            (None, None) => unreachable!(),
        };
        CompareEntry { name, state, left, right }
    }).collect())
}
