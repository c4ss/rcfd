//! Drawing of the case starter (ADR 0139 D8): the "New case from example" dialog and the Project panel's entry. The
//! decisions are in [`crate::case_starter`]; the Case menu and the Project panel ask for the dialog with
//! [`request_open`], so neither needs the dialog's state.

use crate::case_starter::{self, EXAMPLES};
use crate::studio::Studio;
use egui::{Color32, RichText};

const ERR: Color32 = Color32::from_rgb(200, 60, 60);

/// Fields of the open dialog.
#[derive(Debug, Clone, Default)]
pub struct StarterDialog {
    /// Index into [`EXAMPLES`].
    pub selected: usize,
    /// Case name as typed.
    pub name: String,
    /// The user typed the name (until then it follows the proposal for the selected example).
    pub typed: bool,
    /// Example rows drawn in the last frame (ADR 0139 CS12).
    pub rows_drawn: usize,
}

fn request_id() -> egui::Id {
    egui::Id::new("rcfd-studio-case-starter-open")
}

/// Ask for the dialog; it opens in the next call of [`dialog`] (the Case menu entry and the Project panel button).
pub fn request_open(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(request_id(), true));
}

/// The Project panel's entry, drawn above the lists while a project is open: a hint when it has no cases, and the button.
pub fn browser_entry(ui: &mut egui::Ui, st: &Studio) {
    let Some(b) = st.browser() else { return };
    if b.cases.is_empty() {
        ui.label("This project has no cases yet.");
    }
    if ui
        .button("New case from example...")
        .on_hover_text("Copy one of the example cases shipped with rcfd into cases/ of this project")
        .clicked()
    {
        request_open(ui.ctx());
    }
}

/// Draw the dialog while it is open (or was just asked for).
pub fn dialog(ctx: &egui::Context, st: &mut Studio, state: &mut Option<StarterDialog>) {
    if ctx.data_mut(|d| d.remove_temp::<bool>(request_id())).unwrap_or(false) && state.is_none() {
        *state = Some(StarterDialog::default());
    }
    let Some(mut d) = state.take() else { return };
    let mut open = true;
    let mut done = false;
    let cases = st.browser().map(|b| b.cases.clone());
    let descriptions = case_starter::descriptions();
    d.selected = d.selected.min(EXAMPLES.len() - 1);
    egui::Window::new("New case from example")
        .open(&mut open)
        .collapsible(false)
        .default_width(620.0)
        .show(ctx, |ui| {
            let Some(cases) = &cases else {
                ui.label("Open a project first (Project > New project or Open project).");
                return;
            };
            ui.label("Copies an example shipped with rcfd, unchanged, into cases/ of this project. Cheapest first.");
            let mut pick: Option<usize> = None;
            d.rows_drawn = 0;
            egui::ScrollArea::vertical()
                .id_salt("case-starter-examples")
                .max_height(340.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for (i, desc) in descriptions.iter().enumerate() {
                        d.rows_drawn += 1;
                        let title = ui.selectable_label(d.selected == i, RichText::new(&desc.title).strong());
                        let line = ui.add(
                            egui::Label::new(RichText::new(&desc.line).weak())
                                .wrap()
                                .sense(egui::Sense::click()),
                        );
                        if title.clicked() || line.clicked() {
                            pick = Some(i);
                        }
                        ui.add_space(4.0);
                    }
                });
            if let Some(i) = pick {
                d.selected = i;
            }
            let example = &EXAMPLES[d.selected];
            if !d.typed {
                d.name = case_starter::suggest_name(example.stem(), example.ext(), cases);
            }
            ui.separator();
            let mut enter = false;
            ui.horizontal(|ui| {
                ui.label("Name");
                let r = ui.add(egui::TextEdit::singleline(&mut d.name).desired_width(260.0));
                if r.changed() {
                    d.typed = true;
                }
                enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            });
            let check = case_starter::check_name(&d.name, example.ext(), cases);
            match &check {
                Ok(rel) => ui.weak(format!("creates {rel} from the example {}", example.source)),
                Err(m) => ui.colored_label(ERR, m),
            };
            ui.horizontal(|ui| {
                let create = ui.add_enabled(check.is_ok(), egui::Button::new("Create")).clicked();
                if create || (enter && check.is_ok()) {
                    match st.create_case_from_example(example, &d.name) {
                        Ok(_) => done = true,
                        Err(m) => st.say(m),
                    }
                }
                if ui.button("Cancel").clicked() {
                    done = true;
                }
            });
        });
    if open && !done {
        *state = Some(d);
    }
}
