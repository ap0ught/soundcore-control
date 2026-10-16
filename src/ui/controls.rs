use eframe::egui::{self, Align, Layout, RichText};
use openscq30_lib::settings::SettingId;
use soundcore_control::domain::{ControlSetting, ControlValue, DeviceCommand, MultiSelectOption};

use crate::app::SoundcoreApp;
use crate::ui::theme::{MUTED, TEXT};
use crate::ui::widgets::{card, section_title};

impl SoundcoreApp {
    /// Only ever shown when `SoundcoreApp::has_controls` is true (`tab_bar` hides the
    /// Controls tab, and reroutes `active_tab` away from it, otherwise), so at least one of
    /// `daily_controls`/`button_controls` is guaranteed non-empty here.
    pub(crate) fn controls_card(&mut self, ui: &mut egui::Ui) {
        let daily_controls = self.snapshot.daily_controls.clone();
        let button_controls = self.snapshot.button_controls.clone();
        let connected = self.is_connected();
        let mut commands = Vec::new();

        if !daily_controls.is_empty() {
            card(ui, |ui| {
                section_title(ui, "Daily Controls", "Listening and convenience features");
                ui.add_space(12.0);
                let last = daily_controls.len() - 1;
                for (index, control) in daily_controls.iter().enumerate() {
                    if let Some(command) = control_row(ui, control, connected, index < last) {
                        commands.push(command);
                    }
                }
            });
            ui.add_space(14.0);
        }

        if !button_controls.is_empty() {
            card(ui, |ui| {
                section_title(
                    ui,
                    "Button Controls",
                    "Choose an action for each button or gesture",
                );
                ui.add_space(12.0);
                let last = button_controls.len() - 1;
                for (index, control) in button_controls.iter().enumerate() {
                    if let Some(command) = control_row(ui, control, connected, index < last) {
                        commands.push(command);
                    }
                }
            });
        }

        for command in commands {
            self.send(command);
        }
    }
}

fn control_row(
    ui: &mut egui::Ui,
    control: &ControlSetting,
    enabled: bool,
    draw_separator: bool,
) -> Option<DeviceCommand> {
    let mut command = None;
    match &control.value {
        ControlValue::MultiSelect { options, removable } => {
            command = multi_select_block(ui, control, options, *removable, enabled);
        }
        _ => {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&control.label).color(TEXT));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    match &control.value {
                        ControlValue::Toggle(current) => {
                            let mut value = *current;
                            if ui
                                .add_enabled(
                                    enabled,
                                    egui::Button::new(if value { "On" } else { "Off" })
                                        .selected(value),
                                )
                                .clicked()
                            {
                                value = !value;
                                command = Some(DeviceCommand::SetToggle(control.id, value));
                            }
                        }
                        ControlValue::Select {
                            value,
                            options,
                            optional,
                        } => {
                            let selected_label = value
                                .as_deref()
                                .and_then(|selected| {
                                    options
                                        .iter()
                                        .find(|option| option.value == selected)
                                        .map(|option| option.label.as_str())
                                })
                                .unwrap_or("Disabled");
                            egui::ComboBox::from_id_salt(format!("control-{:?}", control.id))
                                .selected_text(selected_label)
                                .width(ui.available_width().min(210.0))
                                .show_ui(ui, |ui| {
                                    if *optional
                                        && ui
                                            .selectable_label(value.is_none(), "Disabled")
                                            .clicked()
                                    {
                                        command = Some(DeviceCommand::SetSelect {
                                            id: control.id,
                                            value: None,
                                            optional: true,
                                        });
                                    }
                                    for option in options {
                                        if ui
                                            .selectable_label(
                                                value.as_deref() == Some(&option.value),
                                                &option.label,
                                            )
                                            .clicked()
                                        {
                                            command = Some(DeviceCommand::SetSelect {
                                                id: control.id,
                                                value: Some(option.value.clone()),
                                                optional: *optional,
                                            });
                                        }
                                    }
                                });
                        }
                        ControlValue::MultiSelect { .. } => {
                            unreachable!("multi-select controls render in their own block above")
                        }
                    }
                });
            });
        }
    }
    if draw_separator {
        ui.add_space(6.0);
        ui.separator();
        ui.add_space(6.0);
    }
    command
}

/// Remembered-host list for multi-select controls (e.g. dual-connection hosts):
/// a checkbox per host (checked = connected) plus a Forget button when the device
/// supports removal. Toggling a checkbox writes back the full desired connected
/// set; the snapshot refreshes from the device on the next state push.
fn multi_select_block(
    ui: &mut egui::Ui,
    control: &ControlSetting,
    options: &[MultiSelectOption],
    removable: bool,
    enabled: bool,
) -> Option<DeviceCommand> {
    let mut command = None;
    ui.label(RichText::new(&control.label).color(TEXT));
    ui.add_space(4.0);
    if options.is_empty() {
        ui.label(RichText::new("No devices").color(MUTED));
        return None;
    }
    // Dual-connection hosts are a fixed two-slot affair (the buds cap multipoint
    // at two hosts), so they render as two connection slots with a host picker
    // each; any other multi-select keeps the generic checkbox list.
    if control.id == SettingId::DualConnectionsDevices {
        return dual_connection_slots(ui, control, options, removable, enabled);
    }
    for option in options {
        ui.horizontal(|ui| {
            let mut selected = option.selected;
            if ui
                .add_enabled(enabled, egui::Checkbox::new(&mut selected, &option.label))
                .changed()
            {
                let values = options
                    .iter()
                    .filter(|current| {
                        if current.value == option.value {
                            selected
                        } else {
                            current.selected
                        }
                    })
                    .map(|current| current.value.clone())
                    .collect();
                command = Some(DeviceCommand::SetMultiSelect {
                    id: control.id,
                    values,
                });
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if removable
                    && ui
                        .add_enabled(enabled, egui::Button::new("Forget"))
                        .clicked()
                {
                    command = Some(DeviceCommand::RemoveMultiSelectOption {
                        id: control.id,
                        value: option.value.clone(),
                    });
                }
                ui.label(RichText::new(&option.value).size(11.0).color(MUTED));
            });
        });
    }
    command
}

/// Two connection slots for dual-connection hosts. Each slot shows its current
/// occupant (or empty) with a picker over all remembered hosts; picking a host
/// connects that slot to it (replacing the occupant), picking empty disconnects
/// the slot. The other slot's occupant is excluded from each picker so both
/// slots can never claim the same host.
fn dual_connection_slots(
    ui: &mut egui::Ui,
    control: &ControlSetting,
    options: &[MultiSelectOption],
    removable: bool,
    enabled: bool,
) -> Option<DeviceCommand> {
    let mut connected = options.iter().filter(|option| option.selected);
    let slots = [connected.next(), connected.next()];
    let mut command = None;
    for (slot, (occupant, other)) in [(slots[0], slots[1]), (slots[1], slots[0])]
        .into_iter()
        .enumerate()
    {
        if let Some(slot_command) = dual_connection_slot(
            ui,
            ConnectionSlot {
                id: control.id,
                options,
                occupant,
                other_occupant: other,
                slot,
                removable,
                enabled,
            },
        ) {
            command = Some(slot_command);
        }
    }
    command
}

struct ConnectionSlot<'a> {
    id: SettingId,
    options: &'a [MultiSelectOption],
    occupant: Option<&'a MultiSelectOption>,
    other_occupant: Option<&'a MultiSelectOption>,
    slot: usize,
    removable: bool,
    enabled: bool,
}

fn dual_connection_slot(
    ui: &mut egui::Ui,
    slot_state: ConnectionSlot<'_>,
) -> Option<DeviceCommand> {
    let ConnectionSlot {
        id,
        options,
        occupant,
        other_occupant,
        slot,
        removable,
        enabled,
    } = slot_state;
    let mut command = None;
    ui.label(RichText::new(format!("Connection {}", slot + 1)).color(TEXT));
    let current = occupant.map(|option| option.value.as_str());
    let selected_label = occupant
        .map(|option| option.label.as_str())
        .unwrap_or("Not connected");
    ui.add_enabled_ui(enabled, |ui| {
        egui::ComboBox::from_id_salt(format!("dual-connection-slot-{slot}"))
            .selected_text(selected_label)
            .width(ui.available_width().min(260.0))
            .show_ui(ui, |ui| {
                ui.set_min_width(240.0);
                if ui
                    .selectable_label(occupant.is_none(), "Not connected")
                    .clicked()
                {
                    let values: Vec<String> = other_occupant
                        .map(|option| option.value.clone())
                        .into_iter()
                        .collect();
                    command = Some(DeviceCommand::SetMultiSelect { id, values });
                }
                for option in options {
                    if Some(option.value.as_str())
                        == other_occupant.map(|other| other.value.as_str())
                    {
                        continue;
                    }
                    if ui
                        .selectable_label(current == Some(option.value.as_str()), &option.label)
                        .clicked()
                    {
                        let mut values: Vec<String> = other_occupant
                            .map(|other| other.value.clone())
                            .into_iter()
                            .collect();
                        values.push(option.value.clone());
                        command = Some(DeviceCommand::SetMultiSelect { id, values });
                    }
                }
            });
    });
    if let Some(occupant) = occupant {
        ui.horizontal(|ui| {
            ui.label(RichText::new(&occupant.value).size(11.0).color(MUTED));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if removable
                    && ui
                        .add_enabled(enabled, egui::Button::new("Forget"))
                        .clicked()
                {
                    command = Some(DeviceCommand::RemoveMultiSelectOption {
                        id,
                        value: occupant.value.clone(),
                    });
                }
            });
        });
    }
    ui.add_space(8.0);
    command
}
