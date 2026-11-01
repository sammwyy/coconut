use super::State;
use coconut_core::{BarEdit, DockConfig};
use coconut_plugin_kit::bar_editor::Target;
use creamui_core::{Point, Rect};

#[derive(Clone)]
pub(super) struct Drag {
    source: Target,
    origin: Point,
    pointer: Point,
    snapshot: DockConfig,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Destination {
    Item { group: usize, at: usize },
    Group(usize),
}

impl State {
    pub(super) fn start_drag(&self, source: Target, point: Point) {
        if self.persistence.busy.peek() {
            return;
        }
        if let Some(draft) = self.draft.peek() {
            self.drag.set(Some(Drag {
                source,
                origin: point,
                pointer: point,
                snapshot: draft.current,
            }));
        }
    }

    pub(super) fn update_drag(&self, point: Point) {
        if let Some(mut drag) = self.drag.peek() {
            drag.pointer = point;
            self.drag.set(Some(drag));
        }
    }

    pub(super) fn destination(&self) -> Option<Destination> {
        let drag = self.drag.get()?;
        if (drag.pointer.x - drag.origin.x).hypot(drag.pointer.y - drag.origin.y) < 4.0 {
            return None;
        }
        if !self.viewport.get().contains(drag.pointer) {
            return None;
        }
        let bounds = self.bounds.borrow();
        let vertical = drag.snapshot.position.is_vertical();
        let after = |rect: &Rect| {
            if vertical {
                drag.pointer.y >= rect.y + rect.height / 2.0
            } else {
                drag.pointer.x >= rect.x + rect.width / 2.0
            }
        };
        if let Target::Item { .. } = drag.source {
            for (target, rect) in bounds.iter() {
                if let Target::Item { group, item } = target {
                    if rect.contains(drag.pointer)
                        && self
                            .clips
                            .borrow()
                            .get(group)
                            .is_some_and(|clip| clip.contains(drag.pointer))
                    {
                        return Some(Destination::Item {
                            group: *group,
                            at: item + usize::from(after(rect)),
                        });
                    }
                }
            }
        }
        for (target, rect) in bounds.iter() {
            if let Target::Group(group) = target {
                if rect.contains(drag.pointer) {
                    return match drag.source {
                        Target::Item { .. } => Some(Destination::Item {
                            group: *group,
                            at: drag.snapshot.sections[*group].islands.len(),
                        }),
                        Target::Group(_) => {
                            Some(Destination::Group(group + usize::from(after(rect))))
                        }
                        _ => None,
                    };
                }
            }
        }
        None
    }

    pub(super) fn finish_drag(&self) {
        let destination = self.destination();
        let Some(drag) = self.drag.peek() else {
            return;
        };
        self.drag.set(None);
        if self
            .draft
            .peek()
            .is_none_or(|draft| draft.current != drag.snapshot)
        {
            return;
        }
        match (drag.source, destination) {
            (
                Target::Item { group, item },
                Some(Destination::Item {
                    group: to_group,
                    at,
                }),
            ) => self.apply(BarEdit::MoveItem {
                group,
                item,
                to_group,
                at,
            }),
            (Target::Group(from), Some(Destination::Group(at))) => {
                self.apply(BarEdit::MoveGroup {
                    from,
                    to: if at > from { at - 1 } else { at },
                });
            }
            _ => {}
        }
    }
}
