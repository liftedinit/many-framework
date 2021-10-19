use derive_builder::Builder;
use minicbor::encode::{Error, Write};
use minicbor::{Decode, Decoder, Encode, Encoder};
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
        pub fn $name( $($fieldname: String,)+ ) -> Self {
            Self {
                code: ErrorCode::$enum,
                message: ($message).to_string(),
                fields: BTreeMap::from_iter(vec![
                    $( (stringify!($fieldname).to_string(), $fieldname) )+
                ]),
            }
        }
    };
}

#[derive(Copy, Clone, Debug)]
pub enum ErrorCode {
    /// Unknown error.
    Unknown,

    /// Invalid method name in the RPC message.
    InvalidMethodName,

    /// Invalid identity (cannot be decoded properly). This is not the same as a signature
    /// mismatch.
    InvalidIdentity,

    /// Application specific codes. Must be 1000 or higher.
    ApplicationSpecific(u32),
}

impl Into<u32> for ErrorCode {
    fn into(self) -> u32 {
        match self {
            ErrorCode::Unknown => 0,
            ErrorCode::InvalidMethodName => 1,
            ErrorCode::InvalidIdentity => 2,
            ErrorCode::ApplicationSpecific(x) => x,
        }
    }
}

impl From<u32> for ErrorCode {
    fn from(v: u32) -> Self {
        match v {
            0 => ErrorCode::Unknown,
            1 => ErrorCode::InvalidMethodName,
            2 => ErrorCode::InvalidIdentity,

            // Application specific error code.
            x if x >= 1000 => Self::ApplicationSpecific(x),

            // Unassociated error code, we just return unknown.
            _ => ErrorCode::Unknown,
        }
    }
}

impl Default for ErrorCode {
    fn default() -> Self {
        ErrorCode::Unknown
    }
}

#[derive(Clone, Debug, Default, Builder)]
#[builder(default)]
pub struct OmniError {
    code: ErrorCode,
    message: String,
    fields: BTreeMap<String, String>,
}

impl OmniError {
    define_error_method!(unknown, Unknown, "Unknown error");
    define_error_method!(
        invalid_method_name,
        InvalidMethodName,
        r#"Invalid method name: "{method}""#,
        (method,)
    );
}

impl Display for OmniError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let message = self.message.as_str();

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

impl std::error::Error for OmniError {}

impl Encode for OmniError {
    fn encode<W: Write>(&self, e: &mut Encoder<W>) -> Result<(), Error<W::Error>> {
        if self.fields.is_empty() {
            e.array(2)?
                .u32(self.code.into())?
                .str(self.message.as_str())?;
        } else {
            e.array(3)?
                .u32(self.code.into())?
                .str(self.message.as_str())?
                .encode(&self.fields)?;
        }
        Ok(())
    }
}

impl<'b> Decode<'b> for OmniError {
    fn decode(d: &mut Decoder<'b>) -> Result<Self, minicbor::decode::Error> {
        let mut builder = OmniErrorBuilder::default();
        match d.array()? {
            None => {
                Err(minicbor::decode::Error::Message(
                    "invalid error array length, need 2 or 3",
                ))?;
            }
            Some(2) => {
                builder.code(d.u32()?.into()).message(d.str()?.to_string());
            }
            Some(3) => {
                builder
                    .code(d.u32()?.into())
                    .message(d.str()?.to_string())
                    .fields(d.decode()?);
            }
            Some(_) => {
                Err(minicbor::decode::Error::Message(
                    "invalid error array length, need 2 or 3",
                ))?;
            }
        }

        Ok(builder.build().unwrap())
    }
}

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

        let e = super::OmniError {
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

        let e = super::OmniError {
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

        let e = super::OmniError {
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

        let e = super::OmniError {
            code: ErrorCode::Unknown,
            message: "/{{}}{{{0}}}{{{a}}}{b}}}{{{2}.".to_string(),
            fields,
        };

        assert_eq!(format!("{}", e), "/{}{ZERO}{}}{TWO.")
    }
}
