use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// A `Clone`-able, type-keyed heterogeneous bag of `Rc`-shared state.
///
/// Replaces `BarActions`'s enumerated closures and the ~20 individually
/// cloned `Rc` integration/signal bindings that used to live as separate
/// locals in `apps/shell/src/lib.rs` (one per integration, one per widget's
/// "fresh state on open"). Any `T: Clone + 'static` — an
/// `Rc<dyn NetworkIntegration>`, a `creamui_reactive::Signal<X>`, a
/// `creamui_widgets::ScrollController` — can be stored and fetched back out
/// by its type, so a plugin only needs to declare which types it wants at
/// the point it uses them, instead of every consumer threading a growing
/// list of named fields through.
///
/// Cloning a `SharedState` clones the `Rc`, not the map: every clone reads
/// and writes the same underlying bag, exactly like the `Rc<RefCell<_>>`
/// integrations it replaces.
#[derive(Clone, Default)]
pub struct SharedState(Rc<RefCell<HashMap<TypeId, Box<dyn Any>>>>);

impl SharedState {
    /// Stores `value`, replacing whatever was previously stored under `T`'s
    /// type (there is at most one value per type in the bag).
    pub fn insert<T: Clone + 'static>(&self, value: T) {
        self.0
            .borrow_mut()
            .insert(TypeId::of::<T>(), Box::new(value));
    }

    /// Returns a clone of the value stored under `T`, if any.
    pub fn get<T: Clone + 'static>(&self) -> Option<T> {
        self.0
            .borrow()
            .get(&TypeId::of::<T>())
            .and_then(|value| value.downcast_ref::<T>())
            .cloned()
    }

    /// Returns the value stored under `T`, lazily creating it with `make`
    /// the first time it's requested. Replaces the "fresh `Signal`/
    /// `ScrollController` per panel open" locals that used to be
    /// hand-created inline in every `open_X` block in
    /// `apps/shell/src/lib.rs` — `make` runs at most once per type, even
    /// across repeated calls, since the created value is stored back into
    /// the bag before returning.
    pub fn get_or_insert_with<T: Clone + 'static>(&self, make: impl FnOnce() -> T) -> T {
        if let Some(value) = self.get::<T>() {
            return value;
        }
        let value = make();
        self.insert(value.clone());
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_returns_none_before_insert() {
        let state = SharedState::default();
        assert_eq!(state.get::<i32>(), None);
    }

    #[test]
    fn insert_then_get_round_trips_by_type() {
        let state = SharedState::default();
        state.insert(42_i32);
        state.insert("hello".to_owned());
        assert_eq!(state.get::<i32>(), Some(42));
        assert_eq!(state.get::<String>(), Some("hello".to_owned()));
    }

    #[test]
    fn clones_share_the_same_backing_map() {
        let state = SharedState::default();
        let clone = state.clone();
        clone.insert(7_u32);
        assert_eq!(state.get::<u32>(), Some(7));
    }

    #[test]
    fn get_or_insert_with_only_calls_the_closure_once() {
        let state = SharedState::default();
        let calls = Rc::new(RefCell::new(0));
        let make_calls = calls.clone();
        let first = state.get_or_insert_with(|| {
            *make_calls.borrow_mut() += 1;
            "first".to_owned()
        });
        let make_calls = calls.clone();
        let second = state.get_or_insert_with(|| {
            *make_calls.borrow_mut() += 1;
            "second".to_owned()
        });
        assert_eq!(first, "first");
        assert_eq!(second, "first");
        assert_eq!(*calls.borrow(), 1);
    }
}
