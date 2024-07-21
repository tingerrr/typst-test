//! Standard library augmentation, i.e. additional functions and values for the
//! typst standard library.
//!
//! # Functions
//! ## `catch`
//! Provides a mechanism to catch panics inside test scripts. Returns an array
//! of strings for each panic.
//! ```typst
//! #let (msg,) = catch(() => {
//!   panic()
//! })
//! ```
//!
//! ## `assert-panic`
//! Provides an assertion that tests if a given closure panicked, panicking if
//! it did not. Takes an optional `message` similar to other `assert` functions.
//! ```typst
//! #assert-panic(() => {}, message: "Did not panic")
//! ```

use comemo::Tracked;
use ecow::EcoString;
use typst::diag::{bail, SourceResult};
use typst::engine::Engine;
use typst::foundations::{func, Array, Context, Func, Module, Repr, Scope, Str, Value};
use typst::{Library, LibraryBuilder};

/// Defines prelude items for the given scope, this is a subset of
/// [`define_test_module`].
pub fn define_prelude(scope: &mut Scope) {
    scope.define_func::<catch>();
    scope.define_func::<assert_panic>();
}

/// Defines test module items for the given scope.
pub fn define_test_module(scope: &mut Scope) {
    define_prelude(scope)
}

/// Retruns a new test module with the items defined by [`define_test_module`].
pub fn test_module() -> Module {
    let mut scope = Scope::new();
    define_test_module(&mut scope);
    Module::new("test", scope)
}

/// Returns a new augmented default standard library. See [`augmented_library`].
pub fn augmented_default_library() -> Library {
    augmented_library(|x| x)
}

/// Returns a new augmented standard library, applying the given closure to the
/// builder.
///
/// The augmented standard library contains a new test module and a few items in
/// the prelude for easier testing.
pub fn augmented_library(builder: impl FnOnce(LibraryBuilder) -> LibraryBuilder) -> Library {
    let mut lib = builder(LibraryBuilder::default()).build();
    let scope = lib.global.scope_mut();

    scope.define_module(test_module());
    define_prelude(scope);

    lib
}

#[func]
fn catch(engine: &mut Engine, context: Tracked<Context>, func: Func) -> Value {
    func.call::<[Value; 0]>(engine, context, [])
        .map(|_| Value::None)
        .unwrap_or_else(|errors| {
            Value::Array(Array::from_iter(
                errors.into_iter().map(|e| Value::Str(Str::from(e.message))),
            ))
        })
}

#[func]
fn assert_panic(
    engine: &mut Engine,
    context: Tracked<Context>,
    func: Func,
    #[named] message: Option<EcoString>,
) -> SourceResult<()> {
    let result = func.call::<[Value; 0]>(engine, context, []);
    let span = func.span();
    if let Ok(val) = result {
        match message {
            Some(message) => bail!(span, "{}", message),
            None => match val {
                Value::None => bail!(
                    span,
                    "Expected panic, closure returned successfully with {}",
                    val.repr(),
                ),
                _ => bail!(span, "Expected panic, closure returned successfully"),
            },
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use typst::eval::Tracer;
    use typst::syntax::Source;

    use super::*;
    use crate::_dev::GlobalTestWorld;
    use crate::compile;

    #[test]
    fn test_catch() {
        let world = GlobalTestWorld::new("".into(), augmented_default_library());
        let source = Source::detached(
            r#"
            #let errors = catch(() => {
                panic()
            })
            #assert.eq(errors.first(), "panicked")
        "#,
        );

        compile::compile(source, &world, &mut Tracer::new()).unwrap();
    }

    #[test]
    fn test_assert_panic() {
        let world = GlobalTestWorld::new("".into(), augmented_default_library());
        let source = Source::detached(
            r#"
            #assert-panic(() => {
                panic()
            })
        "#,
        );

        compile::compile(source, &world, &mut Tracer::new()).unwrap();
    }
}
