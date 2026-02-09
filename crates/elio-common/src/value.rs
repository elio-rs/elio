// #[derive(Debug, Hash, Clone, Eq, PartialEq)]
// pub enum Value {
//     Null,
//     Boolean(bool),
//     Integer(i64),
//     Float(OrderedFloat<f64>),
//     String(String),
//     // composite
//     List(Vec<Value>),
//     // map
//     Map(Vec<(String, Value)>),
//     // structural
//     Node(Box<NodeValue>),
//     Relationship(Box<RelationshipValue>),
//     // Path(Box<PathValue>),
// }

// #[derive(Debug, Hash, Clone, Eq, PartialEq)]
// pub struct NodeValue {
//     pub id: NodeId,
//     pub labels: Vec<LabelId>,
//     pub properties: Vec<(PropertyKeyId, Value)>,
// }

// #[derive(Debug, Hash, Clone, Eq, PartialEq)]
// pub struct RelationshipValue {
//     pub id: RelationshipId,
//     pub reltype: RelationshipTypeId,
//     pub start: NodeId,
//     pub end: NodeId,
//     pub properties: Vec<(PropertyKeyId, Value)>,
// }
