use ecow::{eco_vec, EcoString};

use super::eval::{Context, Error, Eval, TryFromValue, Type, Value};

impl Eval for String {
    fn eval(&self, _ctx: &Context) -> Result<Value, Error> {
        Ok(Value::Str(self.clone()))
    }
}

impl TryFromValue for String {
    fn try_from_value(value: &Value) -> Result<Self, Error> {
        Ok(match value {
            Value::Str(set) => set.clone(),
            _ => {
                return Err(Error::TypeMismatch {
                    expected: eco_vec![Type::Str],
                    found: value.as_type(),
                })
            }
        })
    }
}

impl Eval for EcoString {
    fn eval(&self, _ctx: &Context) -> Result<Value, Error> {
        Ok(Value::Str(self.into()))
    }
}

impl TryFromValue for EcoString {
    fn try_from_value(value: &Value) -> Result<Self, Error> {
        Ok(match value {
            Value::Str(set) => set.into(),
            _ => {
                return Err(Error::TypeMismatch {
                    expected: eco_vec![Type::Str],
                    found: value.as_type(),
                })
            }
        })
    }
}
