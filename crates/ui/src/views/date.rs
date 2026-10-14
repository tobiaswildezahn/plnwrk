//! Date fields with a calendar, and deadline fields with an optional time of day.

use egui::{Id, Popup, PopupCloseBehavior, RichText};
use jiff::civil::Date;

use crate::{dates, text};

/// What happened in a date field this frame.
pub struct DateInput {
    /// The text field.
    pub response: egui::Response,
    /// A day was chosen (or cleared) in the calendar.
    pub picked: bool,
    /// The time field lost the focus.
    pub time_done: bool,
}

impl DateInput {
    /// Whether the value should be taken now: a day was picked, or a field was left.
    pub fn commit(&self) -> bool {
        self.picked || self.time_done || self.response.lost_focus()
    }
}

const MONTHS: [&str; 12] = [
    "Januar",
    "Februar",
    "März",
    "April",
    "Mai",
    "Juni",
    "Juli",
    "August",
    "September",
    "Oktober",
    "November",
    "Dezember",
];
const WEEKDAYS: [&str; 7] = ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"];

/// The first day of the month `months` after (or before) the month of `day`.
fn month_start(day: Date, months: i32) -> Date {
    let first = day.first_of_month();
    first
        .checked_add(jiff::Span::new().months(months))
        .unwrap_or(first)
}

/// The days shown for a month: Monday-aligned rows, `None` for blanks.
pub fn calendar_cells(month: Date) -> Vec<Option<Date>> {
    let first = month.first_of_month();
    let blanks = usize::try_from(first.weekday().to_monday_zero_offset()).unwrap_or(0);
    let mut cells: Vec<Option<Date>> = vec![None; blanks];
    let mut day = Some(first);
    while let Some(current) = day {
        if current.month() != first.month() {
            break;
        }
        cells.push(Some(current));
        day = current.tomorrow().ok();
    }
    cells
}

/// A calendar in a popup; returns the chosen day: `Some(Some(day))`, `Some(None)` for
/// "Leeren", `None` if nothing was chosen.
fn calendar(ui: &mut egui::Ui, id: Id, current: Option<Date>, today: Date) -> Option<Option<Date>> {
    let shown_id = id.with("month");
    let shown: Date = ui
        .ctx()
        .data(|d| d.get_temp(shown_id))
        .unwrap_or_else(|| current.unwrap_or(today).first_of_month());
    let mut choice = None;
    let mut next = shown;
    ui.horizontal(|ui| {
        if ui.small_button("⏴").on_hover_text("Vormonat").clicked() {
            next = month_start(shown, -1);
        }
        let name = MONTHS
            .get(usize::try_from(shown.month() - 1).unwrap_or(0))
            .copied()
            .unwrap_or_default();
        ui.strong(format!("{name} {}", shown.year()));
        if ui.small_button("⏵").on_hover_text("Folgemonat").clicked() {
            next = month_start(shown, 1);
        }
    });
    egui::Grid::new(id.with("days"))
        .num_columns(7)
        .spacing([2.0, 2.0])
        .show(ui, |ui| {
            for weekday in WEEKDAYS {
                ui.label(RichText::new(weekday).small().weak());
            }
            ui.end_row();
            for (index, cell) in calendar_cells(shown).into_iter().enumerate() {
                match cell {
                    Some(day) => {
                        let mut label = RichText::new(format!("{:2}", day.day())).monospace();
                        if day == today {
                            label = label.strong().underline();
                        }
                        if ui
                            .selectable_label(current == Some(day), label)
                            .on_hover_text(dates::format(day))
                            .clicked()
                        {
                            choice = Some(Some(day));
                        }
                    }
                    None => {
                        ui.label("");
                    }
                }
                if index % 7 == 6 {
                    ui.end_row();
                }
            }
        });
    ui.horizontal(|ui| {
        if ui.button(text::TODAY).clicked() {
            choice = Some(Some(today));
        }
        if ui.button(text::CLEAR).clicked() {
            choice = Some(None);
        }
    });
    if next != shown {
        ui.ctx().data_mut(|d| d.insert_temp(shown_id, next));
    }
    choice
}

/// A day field (`TT.MM.JJJJ`) with a calendar button.
pub fn date_input(
    ui: &mut egui::Ui,
    salt: impl std::hash::Hash + std::fmt::Debug,
    value: &mut String,
    today: Date,
    label: Option<Id>,
) -> DateInput {
    let id = Id::new("date-input").with(salt);
    let mut picked = false;
    let response = ui
        .horizontal(|ui| {
            let mut field = ui.add(
                egui::TextEdit::singleline(value)
                    .id_salt(id.with("text"))
                    .hint_text("TT.MM.JJJJ")
                    .desired_width(90.0),
            );
            if let Some(label) = label {
                field = field.labelled_by(label);
            }
            let button = super::icon_button(ui, "📅", text::PICK_DATE);
            let current = dates::parse(value).ok().flatten();
            let popup = Popup::from_toggle_button_response(&button)
                .id(id.with("calendar"))
                .close_behavior(PopupCloseBehavior::CloseOnClickOutside);
            let popup_id = popup.get_id();
            match popup.show(|ui| calendar(ui, id, current, today)) {
                Some(inner) => {
                    if let Some(choice) = inner.inner {
                        *value = dates::format_opt(choice);
                        picked = true;
                        Popup::close_id(ui.ctx(), popup_id);
                    }
                }
                // Closed: next time it opens at the month of the value again.
                None => ui.ctx().data_mut(|d| d.remove::<Date>(id.with("month"))),
            }
            field
        })
        .inner;
    DateInput {
        response,
        picked,
        time_done: false,
    }
}

/// A deadline: day with calendar and, if wanted, a time of day (`HH:MM`). The time field shows
/// when `with_time` is set or a time is already there; otherwise a small button adds it.
pub fn deadline_input(
    ui: &mut egui::Ui,
    salt: impl std::hash::Hash + std::fmt::Debug + Copy,
    day: &mut String,
    time: &mut String,
    today: Date,
    with_time: bool,
    label: Option<Id>,
) -> DateInput {
    ui.horizontal(|ui| {
        let mut input = date_input(ui, salt, day, today, label);
        if input.picked && day.is_empty() {
            // "Leeren" clears the whole deadline, time included.
            time.clear();
        }
        let open_id = Id::new("time-open").with(salt);
        let open = with_time
            || !time.trim().is_empty()
            || ui
                .ctx()
                .data(|d| d.get_temp::<bool>(open_id))
                .unwrap_or(false);
        if open {
            let field = ui.add(
                egui::TextEdit::singleline(time)
                    .id_salt(Id::new("time-input").with(salt))
                    .hint_text(text::TIME_HINT)
                    .desired_width(48.0),
            );
            input.time_done = field.lost_focus();
        } else if super::icon_button(ui, "🕑", text::ADD_TIME).clicked() {
            ui.ctx().data_mut(|d| d.insert_temp(open_id, true));
        }
        input
    })
    .inner
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i16, m: i8, day: i8) -> Date {
        Date::new(y, m, day).unwrap()
    }

    #[test]
    fn months_start_on_monday_columns() {
        // October 2026 starts on a Thursday: three blanks.
        let cells = calendar_cells(d(2026, 10, 14));
        assert_eq!(cells.iter().take_while(|c| c.is_none()).count(), 3);
        assert_eq!(cells.iter().flatten().count(), 31);
        // February 2028 is a leap month, starting on a Tuesday.
        let cells = calendar_cells(d(2028, 2, 10));
        assert_eq!(cells.iter().take_while(|c| c.is_none()).count(), 1);
        assert_eq!(cells.iter().flatten().count(), 29);
        assert_eq!(cells.last().copied().flatten(), Some(d(2028, 2, 29)));
    }

    #[test]
    fn months_step_over_year_ends() {
        assert_eq!(month_start(d(2026, 12, 31), 1), d(2027, 1, 1));
        assert_eq!(month_start(d(2026, 1, 31), -1), d(2025, 12, 1));
        assert_eq!(month_start(d(2026, 1, 31), 1), d(2026, 2, 1));
    }
}
