use super::*;

impl State {
    fn search_value(&self, value: &str) -> Result<(String, SearchOptions), String> {
        let view = self.current().1.borrow().document.view;
        if view.bytes() {
            Ok((
                document::parse_bytes(value, if view == ViewMode::Hex { 16 } else { 2 })?,
                SearchOptions {
                    case_sensitive: true,
                    ..Default::default()
                },
            ))
        } else {
            Ok((value.into(), self.options.peek()))
        }
    }

    pub fn search(&self, query: String) {
        let generation = self.search_generation.get().wrapping_add(1);
        self.search_generation.set(generation);
        *self.matches.borrow_mut() = None;
        *self.search_request.borrow_mut() = None;
        let (query, options) = match self.search_value(&query) {
            Ok(value) => value,
            Err(error) => {
                self.message.set(error);
                return;
            }
        };
        let (id, tab) = self.current();
        let editor = tab.borrow();
        *self.search_request.borrow_mut() = if query.is_empty() {
            None
        } else {
            Some(SearchRequest {
                generation,
                tab: id,
                revision: editor.document.revision,
                text: editor.document.text.clone(),
                query,
                options,
            })
        };
        drop(editor);
        self.dispatch_search();
    }
    pub(super) fn dispatch_search(&self) {
        if self.search_running.get() {
            return;
        }
        let Some(request) = self.search_request.borrow_mut().take() else {
            return;
        };
        self.search_running.set(true);
        let state = self.clone();
        let tag = request.clone();
        self.app.spawn_background(
            move || search_engine::find(&request.text, &request.query, &request.options),
            move |result| {
                state.search_running.set(false);
                if tag.generation == state.search_generation.get() {
                    match result {
                        Ok(ranges) => {
                            let count = ranges.len();
                            *state.matches.borrow_mut() = Some(Matches {
                                tab: tag.tab,
                                revision: tag.revision,
                                ranges: Rc::new(ranges),
                            });
                            state.message.set(format!("{count} matches"));
                        }
                        Err(error) => state.message.set(error),
                    }
                    state.refresh();
                }
                state.dispatch_search();
            },
        );
    }
    pub(super) fn next_match(&self, backwards: bool) {
        let (id, tab) = self.current();
        let mut editor = tab.borrow_mut();
        let results = self.matches.borrow();
        if let Some(matches) = results
            .as_ref()
            .filter(|m| m.tab == id && m.revision == editor.document.revision)
        {
            let range = if backwards {
                let index = matches
                    .ranges
                    .partition_point(|range| range.start < editor.selection().start);
                index
                    .checked_sub(1)
                    .and_then(|index| matches.ranges.get(index))
                    .or_else(|| matches.ranges.last())
            } else {
                let index = matches.ranges.partition_point(|range| {
                    range.start < editor.selection().end
                        || (range.start == editor.cursor && range.is_empty())
                });
                matches.ranges.get(index).or_else(|| matches.ranges.first())
            };
            if let Some(range) = range {
                editor.select(range.clone());
            }
        }
        drop(results);
        drop(editor);
        self.refresh();
    }
    pub(super) fn replace_matches(&self, all: bool) {
        let (id, tab) = self.current();
        let editor = tab.borrow();
        if !editor.editable() {
            drop(editor);
            self.error("Preview is read-only.".into());
            return;
        }
        let range = editor.selection();
        let valid = self.matches.borrow().as_ref().is_some_and(|matches| {
            matches.tab == id
                && matches.revision == editor.document.revision
                && matches.ranges.contains(&range)
        });
        if !all && !valid {
            drop(editor);
            self.next_match(false);
            return;
        }
        let snapshot = editor.document.snapshot();
        let revision = snapshot.revision;
        let view = editor.document.view;
        drop(editor);
        let (replacement, _) = match self.search_value(&self.replacement.peek()) {
            Ok(value) => value,
            Err(error) => {
                self.error(error);
                return;
            }
        };
        let (query, options) = match self.search_value(&self.query.peek()) {
            Ok(value) => value,
            Err(error) => {
                self.error(error);
                return;
            }
        };
        if view == ViewMode::Ascii && !replacement.is_ascii() {
            self.error("Replacement contains non-ASCII characters.".into());
            return;
        }
        let state = self.clone();
        self.busy.update(|busy| *busy += 1);
        self.app.spawn_background(
            move || {
                if all {
                    search_engine::replace_all(&snapshot.text, &query, &replacement, &options)
                } else {
                    search_engine::replace_match(
                        &snapshot.text,
                        range,
                        &query,
                        &replacement,
                        &options,
                    )
                    .map(|text| (text, 1))
                }
            },
            move |result| {
                state.busy.update(|busy| *busy -= 1);
                match result {
                    Ok((text, count)) => {
                        if let Some(tab) = state.tab(id) {
                            let mut editor = tab.borrow_mut();
                            if editor.document.revision == revision && count > 0 {
                                editor.checkpoint();
                                editor.document.text = text;
                                editor.document.touch();
                                let cursor = editor.cursor.min(editor.document.text.len_chars());
                                editor.move_to(cursor, false);
                            }
                        }
                        state.message.set(format!("Replaced {count} matches"));
                        state.search(state.query.peek());
                        state.refresh();
                    }
                    Err(error) => state.error(error),
                }
            },
        );
    }
}
