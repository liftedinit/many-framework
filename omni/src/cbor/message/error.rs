use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::iter::FromIterator;

macro_rules! define_error_method {
    ($name: ident, $enum: ident, $message: literal) => {
        pub fn $name() -> Self {
            Self {
                code: ErrorCode::$enum,
                message: ($message).to_string(),
                ..Default::default()
            }
        }
    };
    ($name: ident, $enum: ident, $message: literal, ($( $fieldname: ident, )+)) => {
        pub fn $name( $($fieldname: String,?)+ ) -> Self {
            Self {
                code: ErrorCode::$enum,
                message: ($message).to_string(),
                fields: BTreeMap::from_iter(vec![
                    $( (stringify!($fieldname), $fieldname) )+
                ]),
            }
        }
    };
}

#[derive(Clone, Debug, Default)]
pub enum ErrorCode {
    Unknown,

    InvalidMethodName,
    UnknownIdentity,
    Custom(u32),
}

#[derive(Clone, Debug, Default)]
pub struct Error {
    code: ErrorCode,
    message: String,
    fields: BTreeMap<String, String>,
}

impl Error {
    define_error_method!(unknown, Unknown, "Unknown error");
    define_error_method!(
        invalid_method_name,
        InvalidMethodName,
        r#"Invalid method name: "{method}""#,
        (method,)
    );
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let message = self.message.as_str();
        let mut i = 0;

        let re = regex::Regex::new(r"\{\{|\}\}|\{[^\}]*\}").unwrap();
        let mut current = 0;

        for mat in re.find_iter(message) {
            let std::ops::Range { start, end } = mat.range();
            f.write_str(&message[current..start])?;
            current = end;

            let s = mat.as_str();
            if s == "{{" {
                f.write_str("{")?;
            } else if s == "}}" {
                f.write_str("}")?;
            } else {
                let field = &message[start + 1..end - 1];
                f.write_str(self.fields.get(field).unwrap_or(&"".to_string()).as_str())?;
            }
        }
        f.write_str(&message[current..])
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use crate::cbor::message::error::ErrorCode;
    use std::collections::BTreeMap;

    #[test]
    fn works() {
        let mut fields = BTreeMap::new();
        fields.insert("0".to_string(), "ZERO".to_string());
        fields.insert("1".to_string(), "ONE".to_string());
        fields.insert("2".to_string(), "TWO".to_string());

        let e = super::Error {
            code: ErrorCode::Unknown,
            message: "Hello {0} and {2}.".to_string(),
            fields,
        };

        assert_eq!(format!("{}", e), "Hello ZERO and TWO.")
    }

    #[test]
    fn works_with_only_replacement() {
        let mut fields = BTreeMap::new();
        fields.insert("0".to_string(), "ZERO".to_string());
        fields.insert("1".to_string(), "ONE".to_string());
        fields.insert("2".to_string(), "TWO".to_string());

        let e = super::Error {
            code: ErrorCode::Unknown,
            message: "{2}".to_string(),
            fields,
        };

        assert_eq!(format!("{}", e), "TWO")
    }

    #[test]
    fn works_for_others() {
        let mut fields = BTreeMap::new();
        fields.insert("0".to_string(), "ZERO".to_string());
        fields.insert("1".to_string(), "ONE".to_string());
        fields.insert("2".to_string(), "TWO".to_string());

        let e = super::Error {
            code: ErrorCode::Unknown,
            message: "@{a}{b}{c}.".to_string(),
            fields,
        };

        assert_eq!(format!("{}", e), "@.")
    }

    #[test]
    fn supports_double_brackets() {
        let mut fields = BTreeMap::new();
        fields.insert("0".to_string(), "ZERO".to_string());
        fields.insert("1".to_string(), "ONE".to_string());
        fields.insert("2".to_string(), "TWO".to_string());

        let e = super::Error {
            code: ErrorCode::Unknown,
            message: "/{{}}{{{0}}}{{{a}}}{b}}}{{{2}.".to_string(),
            fields,
        };

        assert_eq!(format!("{}", e), "/{}{ZERO}{}}{TWO.")
    }
}
