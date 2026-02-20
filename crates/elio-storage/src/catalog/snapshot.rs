use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use elio_common::catalog::{ConstraintCatalogEntry, IndexCatalogEntry, IndexHint, IndexKind};
use elio_common::{LabelId, PropertyKeyId};

#[derive(Debug, Clone)]
pub(crate) enum CatalogChange {
    UpsertConstraint(ConstraintCatalogEntry),
    DeleteConstraint { name: Arc<str>, label_id: LabelId },
    UpsertIndex(IndexCatalogEntry),
    DeleteIndex { name: Arc<str>, label_id: LabelId },
}

/// Catalog entries that will be applied to global catalog snapshot
/// This include all catalog entries except function catalog entry.
#[derive(Debug, Default, Clone)]
pub(crate) struct DurableCatalogSnapshot {
    constraints_by_name: HashMap<Arc<str>, ConstraintCatalogEntry>,
    constraints_by_label: HashMap<LabelId, HashSet<Arc<str>>>,
    indexes_by_name: HashMap<Arc<str>, IndexCatalogEntry>,
    indexes_by_label: HashMap<LabelId, HashSet<Arc<str>>>,
}

impl DurableCatalogSnapshot {
    pub(crate) fn from_entries(constraints: Vec<ConstraintCatalogEntry>, indexes: Vec<IndexCatalogEntry>) -> Self {
        let mut snapshot = Self::default();
        for entry in constraints {
            let name = entry.name.clone();
            snapshot.constraints_by_name.insert(name.clone(), entry.clone());
            snapshot.add_constraint_label_name(entry.label_or_rel_id, name);
        }
        for entry in indexes {
            let name = entry.name.clone();
            snapshot.indexes_by_name.insert(name.clone(), entry.clone());
            snapshot.add_index_label_name(entry.label_id, name);
        }
        snapshot
    }

    pub(crate) fn constraint_exists(&self, name: &str) -> bool {
        self.constraints_by_name.contains_key(name)
    }

    pub(crate) fn get_constraint(&self, name: &str) -> Option<ConstraintCatalogEntry> {
        self.constraints_by_name.get(name).cloned()
    }

    pub(crate) fn get_constraints_for_label(&self, label_id: LabelId) -> Vec<ConstraintCatalogEntry> {
        self.constraints_by_label
            .get(&label_id)
            .into_iter()
            .flatten()
            .filter_map(|name| self.constraints_by_name.get(name).cloned())
            .collect()
    }

    pub(crate) fn get_indexes_for_label(&self, label_id: LabelId) -> Vec<IndexCatalogEntry> {
        self.indexes_by_label
            .get(&label_id)
            .into_iter()
            .flatten()
            .filter_map(|name| self.indexes_by_name.get(name).cloned())
            .collect()
    }

    pub(crate) fn find_unique_index(&self, label_id: LabelId, property_key_ids: &[PropertyKeyId]) -> Option<IndexHint> {
        for index in self.get_indexes_for_label(label_id) {
            if matches!(index.index_kind, IndexKind::Unique)
                && index.property_key_ids.len() <= property_key_ids.len()
                && index.property_key_ids.iter().all(|p| property_key_ids.contains(p))
            {
                return Some(IndexHint {
                    index_name: index.name,
                    label_id: index.label_id,
                    property_key_ids: index.property_key_ids,
                });
            }
        }
        None
    }

    pub(crate) fn apply_changes(&self, changes: &[CatalogChange]) -> Self {
        let mut next = self.clone();
        for change in changes {
            match change {
                CatalogChange::UpsertConstraint(entry) => {
                    let name = entry.name.clone();
                    if let Some(prev) = next.constraints_by_name.insert(name.clone(), entry.clone()) {
                        next.remove_constraint_label_name(prev.label_or_rel_id, &name);
                    }
                    next.add_constraint_label_name(entry.label_or_rel_id, name);
                }
                CatalogChange::DeleteConstraint { name, label_id } => {
                    if let Some(prev) = next.constraints_by_name.remove(name) {
                        next.remove_constraint_label_name(prev.label_or_rel_id, name);
                    } else {
                        next.remove_constraint_label_name(*label_id, name);
                    }
                }
                CatalogChange::UpsertIndex(entry) => {
                    let name = entry.name.clone();
                    if let Some(prev) = next.indexes_by_name.insert(name.clone(), entry.clone()) {
                        next.remove_index_label_name(prev.label_id, &name);
                    }
                    next.add_index_label_name(entry.label_id, name);
                }
                CatalogChange::DeleteIndex { name, label_id } => {
                    if let Some(prev) = next.indexes_by_name.remove(name) {
                        next.remove_index_label_name(prev.label_id, name);
                    } else {
                        next.remove_index_label_name(*label_id, name);
                    }
                }
            }
        }
        next
    }

    fn add_constraint_label_name(&mut self, label_id: LabelId, name: Arc<str>) {
        self.constraints_by_label.entry(label_id).or_default().insert(name);
    }

    fn remove_constraint_label_name(&mut self, label_id: LabelId, name: &Arc<str>) {
        let Some(names) = self.constraints_by_label.get_mut(&label_id) else {
            return;
        };
        names.remove(name);
        if names.is_empty() {
            self.constraints_by_label.remove(&label_id);
        }
    }

    fn add_index_label_name(&mut self, label_id: LabelId, name: Arc<str>) {
        self.indexes_by_label.entry(label_id).or_default().insert(name);
    }

    fn remove_index_label_name(&mut self, label_id: LabelId, name: &Arc<str>) {
        let Some(names) = self.indexes_by_label.get_mut(&label_id) else {
            return;
        };
        names.remove(name);
        if names.is_empty() {
            self.indexes_by_label.remove(&label_id);
        }
    }
}
