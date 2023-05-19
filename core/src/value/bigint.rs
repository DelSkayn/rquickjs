use crate::{qjs, Coerced, Ctx, Error, FromJs, Result, String, Value};
use std::string::String as StdString;

#[cfg(feature = "bigint")]
use num_bigint::BigInt as NumBigInt;

/// Rust representation of a javascript big int.
#[derive(Debug, Clone, PartialEq)]
#[repr(transparent)]
pub struct BigInt<'js>(pub(crate) Value<'js>);

impl<'js> BigInt<'js> {
    pub fn from_i64(ctx: Ctx<'js>, v: i64) -> Result<Self> {
        unsafe {
            let v = ctx.handle_exception(qjs::JS_NewBigInt64(ctx.as_ptr(), v))?;
            Ok(BigInt(Value::from_js_value(ctx, v)))
        }
    }

    pub fn from_u64(ctx: Ctx<'js>, v: u64) -> Result<Self> {
        unsafe {
            let v = ctx.handle_exception(qjs::JS_NewBigUint64(ctx.as_ptr(), v))?;
            Ok(BigInt(Value::from_js_value(ctx, v)))
        }
    }

    pub fn to_i64(self) -> Result<i64> {
        unsafe {
            let mut res: i64 = 0;
            if qjs::JS_ToInt64Ext(self.0.ctx.as_ptr(), &mut res, self.0.value) == -1 {
                return Err(Error::Unknown);
            }
            Ok(res)
        }
    }
}

#[cfg(feature = "bigint")]
impl<'js> BigInt<'js> {
    pub fn to_num_bigint(self) -> Result<NumBigInt> {
        let str = Coerced::<StdString>::from_js(self.0.ctx, self.0)?.0;
        // Quickjs should always return a valid radix 10 digit.
        NumBigInt::parse_bytes(str.as_bytes(), 10).ok_or(Error::Unknown)
    }
}

#[cfg(test)]
mod test {
    use crate::*;
    #[test]
    fn from_javascript() {
        test_with(|ctx| {
            let s: BigInt = ctx.eval(format!("{}n", i64::MAX)).unwrap();
            assert_eq!(s.to_i64().unwrap(), i64::MAX);
            let s: BigInt = ctx.eval(format!("{}n", i64::MIN)).unwrap();
            assert_eq!(s.to_i64().unwrap(), i64::MIN);
        })
    }

    #[test]
    fn to_javascript() {
        test_with(|ctx| {
            let bigint = BigInt::from_i64(ctx, i64::MAX).unwrap();
            let func: Function = ctx
                .eval(format!(
                    "x => {{
                if( x != {}n){{
                    throw 'error'
                }}
            }}",
                    i64::MAX
                ))
                .unwrap();
            func.call::<_, ()>((bigint,)).unwrap();
            let bigint = BigInt::from_i64(ctx, i64::MIN).unwrap();
            let func: Function = ctx
                .eval(format!(
                    "x => {{
                if( x != {}n){{
                    throw 'error'
                }}
            }}",
                    i64::MIN
                ))
                .unwrap();
            func.call::<_, ()>((bigint,)).unwrap();
        })
    }
}
