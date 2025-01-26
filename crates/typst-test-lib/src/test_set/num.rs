use ecow::eco_vec;

use super::eval::{Context, Error, Eval, TryFromValue, Type, Value};

impl Eval for usize {
    fn eval(&self, _ctx: &Context) -> Result<Value, Error> {
        Ok(Value::Num(*self))
    }
}

impl TryFromValue for usize {
    fn try_from_value(value: &Value) -> Result<Self, Error> {
        Ok(match value {
            Value::Num(set) => *set,
            _ => {
                return Err(Error::TypeMismatch {
                    expected: eco_vec![Type::Num],
                    found: value.as_type(),
                })
            }
        })
    }
}
