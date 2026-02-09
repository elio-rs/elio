use elio_common::variable::VariableName;
use itertools::Itertools;
use pretty_xmlish::{Pretty, PrettyConfig, XmlNode};

use crate::expr::Expr;
use crate::ir::order::SortItem;

pub(crate) fn pretty_display_iter<T: std::fmt::Display>(iter: impl Iterator<Item = T>) -> Pretty<'static> {
    Pretty::Array(iter.map(|x| Pretty::display(&x)).collect_vec())
}

pub(crate) fn pretty_project_items<'a>(items: impl Iterator<Item = (&'a VariableName, &'a Expr)>) -> Pretty<'static> {
    Pretty::Array(
        items
            .map(|(k, v)| Pretty::display(&format!("{} AS {}", k, v.pretty())))
            .collect::<Vec<_>>(),
    )
}

pub(crate) fn pretty_order_items(order_by: &[SortItem]) -> Pretty<'_> {
    Pretty::Array(order_by.iter().map(Pretty::display).collect::<Vec<_>>())
}

pub(crate) fn _xmlnode_to_string<'a>(xml: XmlNode<'a>) -> String {
    let record = Pretty::Record(xml);
    let mut config = PrettyConfig {
        indent: 3,
        width: 2048,
        need_boundaries: false,
        reduced_spaces: true,
    };
    let mut output = String::with_capacity(2048);
    config.unicode(&mut output, &record);
    output
}
