use crate::protocol::Attribute;

pub const BASE_SERVER: Attribute = Attribute {
    id: 0,
    // endpoints: Box::new(["status".to_string(), "echo".to_string()]).into_vec(),
    endpoints: &["status", "echo"],
};
