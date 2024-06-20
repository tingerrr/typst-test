use id::Identifier;

use crate::store;

pub mod id;

// TODO: this will contain actual storage of intermediate steps of the test process
pub struct Test {
    handle: store::test::Test,
}

impl Test {
    /// Returns a reference to the identifier of this test.
    pub fn id(&self) -> &Identifier {
        self.handle.id()
    }

    /// Returns a reference to the reference kind of this test.
    pub fn ref_kind(&self) -> Option<&ReferenceKind> {
        self.handle.ref_kind()
    }

    /// Returns whether this test is compared to a reference script.
    pub fn is_ephemeral(&self) -> bool {
        self.handle.is_ephemeral()
    }

    /// Returns whether this test is compared to reference images directly.
    pub fn is_persistent(&self) -> bool {
        self.handle.is_persistent()
    }

    /// Returns whether this test is not compared, but only compiled.
    pub fn is_compile_only(&self) -> bool {
        self.handle.is_compile_only()
    }

    /// Returns whether this test is marked as ignored.
    pub fn is_ignored(&self) -> bool {
        self.handle.is_ignored()
    }
}

/// The kind of a [`Test`]'s reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReferenceKind {
    /// Ephemeral references are references which are compiled on the fly from a script.
    Ephemeral,

    /// Persistent references are pre compiled and fetched for comparison.
    Persistent,
}
