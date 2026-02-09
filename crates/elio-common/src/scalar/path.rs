use super::*;

#[derive(Debug, Clone)]
pub struct VirtualPath {
    pub nodes: Arc<ArrayImpl>,
    pub rels: Arc<ArrayImpl>,
}

impl PartialEq for VirtualPath {
    fn eq(&self, other: &Self) -> bool {
        self.node_iter().eq(other.node_iter()) && self.rel_iter().eq(other.rel_iter())
    }
}
impl Eq for VirtualPath {}

impl Hash for VirtualPath {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.node_iter().for_each(|node| node.hash(state));
        self.rel_iter().for_each(|rel| rel.hash(state));
    }
}

impl ScalarPartialOrd for VirtualPath {
    fn scalar_partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.as_scalar_ref().scalar_partial_cmp(&other.as_scalar_ref())
    }
}

impl ScalarVTable for VirtualPath {
    type RefType<'a> = VirtualPathRef<'a>;

    fn as_scalar_ref(&self) -> Self::RefType<'_> {
        VirtualPathRef {
            nodes: self.nodes.as_ref(),
            node_start: 0,
            node_end: self.nodes.len(),
            rels: self.rels.as_ref(),
            rel_start: 0,
            rel_end: self.rels.len(),
        }
    }
}

impl VirtualPath {
    pub fn node_iter(&self) -> impl ExactSizeIterator<Item = Option<NodeId>> + '_ {
        self.nodes.as_virtual_node().unwrap().iter()
    }

    pub fn rel_iter(&self) -> impl ExactSizeIterator<Item = Option<RelValueRef<'_>>> + '_ {
        self.rels.as_rel().unwrap().iter()
    }
}

impl std::fmt::Display for VirtualPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut buf = String::new();
        format_virtual_path(&mut buf, self.node_iter(), self.rel_iter())?;
        write!(f, "{}", buf)
    }
}

fn format_path<'a>(
    f: &mut String,
    mut nodes: impl ExactSizeIterator<Item = Option<NodeValueRef<'a>>>,
    mut rels: impl ExactSizeIterator<Item = Option<RelValueRef<'a>>>,
) -> std::fmt::Result {
    use std::fmt::Write;
    assert_eq!(nodes.len(), rels.len() + 1);

    // SAFETY
    //  PATH element must not be null
    write!(f, "({})", nodes.next().unwrap().unwrap())?;
    let len = rels.len();

    for _ in 0..len {
        let rel = rels.next().unwrap().unwrap();
        let rhs = nodes.next().unwrap().unwrap().id;
        let (ldir, rdir) = match rel.relative_dir(rhs) {
            Some(RelDirection::Out) => ("<-", "-"),
            Some(RelDirection::In) => ("-", "->"),
            _ => unreachable!("path element must be connected"),
        };
        write!(f, "{ldir}[{}]{rdir}({})", rel, rhs)?;
    }
    Ok(())
}

fn format_virtual_path<'a>(
    f: &mut String,
    mut nodes: impl ExactSizeIterator<Item = Option<NodeId>>,
    mut rels: impl ExactSizeIterator<Item = Option<RelValueRef<'a>>>,
) -> std::fmt::Result {
    use std::fmt::Write;
    assert_eq!(nodes.len(), rels.len() + 1);

    // SAFETY
    //  PATH element must not be null
    write!(f, "({})", nodes.next().unwrap().unwrap())?;
    let len = rels.len();

    for _ in 0..len {
        let rel = rels.next().unwrap().unwrap();
        let rhs = nodes.next().unwrap().unwrap();
        let (ldir, rdir) = match rel.relative_dir(rhs) {
            Some(RelDirection::Out) => ("<-", "-"),
            Some(RelDirection::In) => ("-", "->"),
            _ => unreachable!("path element must be connected"),
        };
        write!(f, "{ldir}[{}]{rdir}({})", rel, rhs)?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub struct VirtualPathRef<'a> {
    pub nodes: &'a ArrayImpl, // virtual node
    pub node_start: usize,
    pub node_end: usize,
    pub rels: &'a ArrayImpl, // rel
    pub rel_start: usize,
    pub rel_end: usize,
}

impl<'a> std::hash::Hash for VirtualPathRef<'a> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let node_iter = self.node_iter();
        let rel_iter = self.rel_iter();
        for node in node_iter {
            node.hash(state);
        }
        for rel in rel_iter {
            rel.hash(state);
        }
    }
}

impl<'a> ScalarPartialOrd for VirtualPathRef<'a> {
    fn scalar_partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let len1 = self.nodes.len();
        let len2 = other.nodes.len();
        let min_len = len1.min(len2);

        let mut n1_iter = self.node_iter();
        let mut n2_iter = other.node_iter();
        let mut r1_iter = self.rel_iter();
        let mut r2_iter = other.rel_iter();

        for i in 0..min_len {
            // # SAFETY
            //    element in path is always non-null
            let n1 = n1_iter.next().unwrap().expect("node is null in path lhs");
            let n2 = n2_iter.next().unwrap().expect("node is null in path rhs");
            match n1.partial_cmp(&n2) {
                Some(std::cmp::Ordering::Equal) => {}
                ord => return ord,
            }

            if i < min_len - 1 {
                let r1 = r1_iter.next().unwrap().expect("rel is null in path lhs");
                let r2 = r2_iter.next().unwrap().expect("rel is null in path rhs");
                match r1.scalar_partial_cmp(&r2) {
                    Some(std::cmp::Ordering::Equal) => {}
                    ord => return ord,
                }
            }
        }
        len1.partial_cmp(&len2)
    }
}

impl<'a> ScalarRefVTable<'a> for VirtualPathRef<'a> {
    type ScalarType = VirtualPath;

    fn to_owned_scalar(&self) -> Self::ScalarType {
        VirtualPath {
            nodes: Arc::new(
                self.nodes
                    .as_virtual_node()
                    .unwrap()
                    .slice(self.node_start, self.node_end)
                    .into(),
            ),
            rels: Arc::new(self.rels.as_rel().unwrap().slice(self.rel_start, self.rel_end).into()),
        }
    }
}

impl<'a> VirtualPathRef<'a> {
    pub fn node_list_ref(&'a self) -> ListValueRef<'a> {
        ListValueRef::Index {
            child: self.nodes,
            start: self.node_start,
            end: self.node_end,
        }
    }

    pub fn rel_list_ref(&'a self) -> ListValueRef<'a> {
        ListValueRef::Index {
            child: self.rels,
            start: self.rel_start,
            end: self.rel_end,
        }
    }

    pub fn node_iter(&'a self) -> impl ExactSizeIterator<Item = Option<NodeId>> {
        (self.node_start..self.node_end).map(|idx| self.nodes.as_virtual_node().unwrap().get(idx))
    }

    pub fn rel_iter(&'a self) -> impl ExactSizeIterator<Item = Option<RelValueRef<'a>>> {
        (self.rel_start..self.rel_end).map(|idx| self.rels.as_rel().unwrap().get(idx))
    }
}

impl<'a> std::fmt::Display for VirtualPathRef<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut buf = String::new();
        format_virtual_path(&mut buf, self.node_iter(), self.rel_iter())?;
        write!(f, "{}", buf)
    }
}

impl<'a> PartialEq for VirtualPathRef<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.node_iter().eq(other.node_iter()) && self.rel_iter().eq(other.rel_iter())
    }
}

impl<'a> Eq for VirtualPathRef<'a> {}

#[derive(Debug, Clone)]
pub struct PathValue {
    pub nodes: Arc<ArrayImpl>, // node array
    pub rels: Arc<ArrayImpl>,
}

impl PartialEq for PathValue {
    fn eq(&self, other: &Self) -> bool {
        self.node_iter().eq(other.node_iter()) && self.rel_iter().eq(other.rel_iter())
    }
}

impl Eq for PathValue {}

impl Hash for PathValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.node_iter().for_each(|node| node.hash(state));
        self.rel_iter().for_each(|rel| rel.hash(state));
    }
}

impl ScalarPartialOrd for PathValue {
    fn scalar_partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.as_scalar_ref().scalar_partial_cmp(&other.as_scalar_ref())
    }
}

impl ScalarVTable for PathValue {
    type RefType<'a> = PathValueRef<'a>;

    fn as_scalar_ref(&self) -> Self::RefType<'_> {
        PathValueRef {
            nodes: &self.nodes,
            node_start: 0,
            node_end: self.nodes.len(),
            rels: &self.rels,
            rel_start: 0,
            rel_end: self.rels.len(),
        }
    }
}

impl PathValue {
    pub fn node_iter(&self) -> impl ExactSizeIterator<Item = Option<NodeValueRef<'_>>> {
        let node_array = self.nodes.as_node().unwrap();
        (0..self.nodes.len()).map(move |idx| node_array.get(idx))
    }

    pub fn rel_iter(&self) -> impl ExactSizeIterator<Item = Option<RelValueRef<'_>>> {
        let rel_array = self.rels.as_rel().unwrap();
        (0..self.rels.len()).map(move |idx| rel_array.get(idx))
    }
}

impl std::fmt::Display for PathValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut buf = String::new();
        format_path(&mut buf, self.node_iter(), self.rel_iter())?;
        write!(f, "{}", buf)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PathValueRef<'a> {
    pub nodes: &'a ArrayImpl, // node array
    pub node_start: usize,
    pub node_end: usize,
    pub rels: &'a ArrayImpl, // rel array
    pub rel_start: usize,
    pub rel_end: usize,
}

impl<'a> std::hash::Hash for PathValueRef<'a> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let node = self.node_iter();
        let rel = self.rel_iter();
        for node in node {
            node.hash(state);
        }
        for rel in rel {
            rel.hash(state);
        }
    }
}

impl<'a> ScalarRefVTable<'a> for PathValueRef<'a> {
    type ScalarType = PathValue;

    fn to_owned_scalar(&self) -> Self::ScalarType {
        PathValue {
            nodes: Arc::new(
                self.nodes
                    .as_node()
                    .unwrap()
                    .slice(self.node_start, self.node_end)
                    .into(),
            ),
            rels: Arc::new(self.rels.as_rel().unwrap().slice(self.rel_start, self.rel_end).into()),
        }
    }
}

impl<'a> PathValueRef<'a> {
    pub fn node_list_ref(&'a self) -> ListValueRef<'a> {
        ListValueRef::Index {
            child: self.nodes,
            start: self.node_start,
            end: self.node_end,
        }
    }

    pub fn rel_list_ref(&'a self) -> ListValueRef<'a> {
        ListValueRef::Index {
            child: self.rels,
            start: self.rel_start,
            end: self.rel_end,
        }
    }

    pub fn node_iter(&'a self) -> impl ExactSizeIterator<Item = Option<NodeValueRef<'a>>> {
        // TODO(pgao): avoid downcast in iter
        (self.node_start..self.node_end).map(|idx| self.nodes.as_node().unwrap().get(idx))
    }

    pub fn rel_iter(&'a self) -> impl ExactSizeIterator<Item = Option<RelValueRef<'a>>> {
        // TODO(pgao): avoid downcast in iter
        (self.rel_start..self.rel_end).map(|idx| self.rels.as_rel().unwrap().get(idx))
    }
}

impl<'a> PartialEq for PathValueRef<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.node_iter().eq(other.node_iter()) && self.rel_iter().eq(other.rel_iter())
    }
}

impl<'a> Eq for PathValueRef<'a> {}

impl<'a> std::fmt::Display for PathValueRef<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut buf = String::new();
        format_path(&mut buf, self.node_iter(), self.rel_iter())?;
        write!(f, "{}", buf)
    }
}

impl<'a> ScalarPartialOrd for PathValueRef<'a> {
    fn scalar_partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let len1 = self.nodes.len();
        let len2 = other.nodes.len();
        let min_len = len1.min(len2);

        let mut n1_iter = self.node_iter();
        let mut n2_iter = other.node_iter();
        let mut r1_iter = self.rel_iter();
        let mut r2_iter = other.rel_iter();

        for i in 0..min_len {
            // # SAFETY
            //    element in path is always non-null
            let n1 = n1_iter.next().unwrap().expect("node is null in path lhs");
            let n2 = n2_iter.next().unwrap().expect("node is null in path rhs");
            match n1.scalar_partial_cmp(&n2) {
                Some(std::cmp::Ordering::Equal) => {}
                ord => return ord,
            }

            if i < min_len - 1 {
                let r1 = r1_iter.next().unwrap().expect("rel is null in path lhs");
                let r2 = r2_iter.next().unwrap().expect("rel is null in path rhs");
                match r1.scalar_partial_cmp(&r2) {
                    Some(std::cmp::Ordering::Equal) => {}
                    ord => return ord,
                }
            }
        }
        len1.partial_cmp(&len2)
    }
}
