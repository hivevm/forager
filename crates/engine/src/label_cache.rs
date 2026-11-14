//! The address of a label of the render, and the lists of the label cache, how they grow and the
//! `u32` they are indexed by, which the labels and their styles share below both.

use forager_style_ir::{Label, Map};

/// A label of the render by its layer's index, its class's, and its index in the class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct LabelAt(pub(crate) usize, pub(crate) usize, pub(crate) usize);

impl LabelAt {
    /// The label of `map` this finds.
    pub(crate) fn label(self, map: &Map) -> &Label {
        &map.layers[self.0].classes[self.1].labels[self.2]
    }
}

/// Makes room in `list`, a list of the label cache, for `more` entries as they are held, growing
/// it by a quarter of its capacity, or by what they need where that is more. A list that
/// doubled as it grew would hold up to three times what it fills while it moved, past what Q-9
/// bounds per label; one sized ahead for the features of a layer would hold room for labels a
/// feature's `EXPRESSION`, its empty text, or its layout leaves out. Grown by half, the lists
/// hold 400 bytes per label without style just past a step, at 15,586 labels; by a quarter, 363
/// at 5,262, the most measured, and 392 at 1,109, where the render's fixed cost counts more
/// per label. Each entry moves about four times as the list grows.
pub(crate) fn make_room<T>(list: &mut Vec<T>, more: usize) {
    if list.capacity() - list.len() < more {
        list.reserve_exact(more.max(list.capacity() / 4).max(MIN_ROOM));
    }
}

/// Pushes `entry` onto `list`, a list of the label cache, grown as [`make_room`] grows it.
pub(crate) fn push<T>(list: &mut Vec<T>, entry: T) {
    make_room(list, 1);
    list.push(entry);
}

/// The fewest entries a list of the label cache grows by, so that a small one is not moved at
/// each entry.
pub(crate) const MIN_ROOM: usize = 8;

/// An index into a render's labels, their texts, paths, or glyph caches, as the `u32` they are
/// filed by.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the labels cached are held to MAX_LABELS, and their texts, paths, and defs with them"
)]
pub(crate) fn index(i: usize) -> u32 {
    i as u32
}
