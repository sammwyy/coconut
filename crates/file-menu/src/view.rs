use super::*;

impl FileMenuController {
    pub fn overlays(&self, size: Size) -> Vec<BoxedWidget> {
        let mut overlays = Vec::new();
        if let Some(menu) = self.menu.overlay(size) {
            overlays.push(menu);
        }
        if let Some(panel) = self.panel.get() {
            let dismiss = self.clone();
            let dialog = match panel {
                Panel::Information { title, body } => {
                    Dialog::new(title, body, move || dismiss.dismiss()).dismiss_action("Close")
                }
                Panel::Form(form) => {
                    let (title, action) = match form.kind {
                        FormKind::NewFile => ("New file", "Create"),
                        FormKind::NewFolder => ("New folder", "Create"),
                        FormKind::Rename => ("Rename", "Rename"),
                    };
                    let submit = self.clone();
                    let input = TextInput::controlled(&self.name)
                        .on_submit(move || submit.submit())
                        .layout(creamui_core::layout::Style {
                            size: creamui_core::layout::Size {
                                width: creamui_core::layout::Dimension::Percent(1.0),
                                height: creamui_core::layout::Dimension::Length(38.0),
                            },
                            ..Default::default()
                        });
                    let mut content = DialogContent::new().compact().child(Box::new(input));
                    if let Some(error) = self.error.get() {
                        content = content.child(Box::new(DialogMessage::new(error)));
                    }
                    let submit = self.clone();
                    let mut dialog = Dialog::custom(move || dismiss.dismiss())
                        .header(Box::new(DialogHeader::new(title).compact()))
                        .content(Box::new(content));
                    if self.busy.get() {
                        dialog = dialog.content(Box::new(DialogMessage::new("Working…")));
                    } else {
                        dialog = dialog.dismiss_action("Cancel").action(
                            action,
                            ButtonVariant::Primary,
                            move || submit.submit(),
                        );
                    }
                    dialog
                }
            };
            overlays.push(Box::new(dialog));
        }
        overlays
    }
}
