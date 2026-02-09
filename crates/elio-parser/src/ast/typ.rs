use derive_more::Display;

#[derive(Debug, Display)]
pub enum DataType {
    #[display("INTEGER")]
    Integer,
    #[display("FLOAT")]
    Float,
    #[display("STRING")]
    String,
    #[display("BOOLEAN")]
    Boolean,
}
