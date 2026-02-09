use std::sync::atomic::AtomicUsize;

use elio_common::variable::VariableName;

#[derive(Debug, Default)]
pub struct VariableGenerator {
    next_id: AtomicUsize,
}

impl VariableGenerator {
    pub fn named(&self, hint: &str) -> VariableName {
        let id = self.next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        format!("{}@{}", Self::norm(hint), id).into()
    }

    pub fn unnamed(&self) -> VariableName {
        let id = self.next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        format!("anon@{}", id).into()
    }

    // normalize the str, made it available as variable, where only alphbet, number and _ allowed
    fn norm(hint: &str) -> String {
        hint.chars().filter(|x| x.is_alphanumeric() || *x == '_').collect()
    }
}
