use crate::bluetooth::{BluerDeviceStatus, BluerRequest, BluerState};
use cosmic::applet::APPLET_BUTTON_THEME;
use cosmic::iced_style;
use cosmic::{
    applet::CosmicAppletHelper,
    iced::{
        wayland::{
            popup::{destroy_popup, get_popup},
            SurfaceIdWrapper,
        },
        widget::{column, container, row, scrollable, text, Column},
        Alignment, Application, Color, Command, Length, Subscription,
    },
    iced_native::{
        alignment::{Horizontal, Vertical},
        layout::Limits,
        renderer::BorderRadius,
        window,
    },
    iced_style::{application, button::StyleSheet},
    theme::{Button, Svg},
    widget::{button, divider, icon, toggler},
    Element, Theme,
};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc::Sender;

use crate::bluetooth::{bluetooth_subscription, BluerDevice, BluerEvent};
use crate::{config, fl};

pub fn run() -> cosmic::iced::Result {
    let helper = CosmicAppletHelper::default();
    CosmicBluetoothApplet::run(helper.window_settings())
}

#[derive(Default)]
struct CosmicBluetoothApplet {
    icon_name: String,
    theme: Theme,
    popup: Option<window::Id>,
    id_ctr: u32,
    applet_helper: CosmicAppletHelper,
    bluer_state: BluerState,
    bluer_sender: Option<Sender<BluerRequest>>,
    // UI state
    show_visible_devices: bool,
    request_confirmation: Option<(BluerDevice, String, Sender<bool>)>,
}

#[derive(Debug, Clone)]
enum Message {
    TogglePopup,
    ToggleVisibleDevices(bool),
    Errored(String),
    Ignore,
    BluetoothEvent(BluerEvent),
    Request(BluerRequest),
    Cancel,
    Confirm,
}

impl Application for CosmicBluetoothApplet {
    type Message = Message;
    type Theme = Theme;
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        (
            CosmicBluetoothApplet {
                icon_name: "bluetooth-symbolic".to_string(),
                ..Default::default()
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        config::APP_ID.to_string()
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::TogglePopup => {
                if let Some(p) = self.popup.take() {
                    return destroy_popup(p);
                } else {
                    // TODO request update of state maybe
                    self.id_ctr += 1;
                    let new_id = window::Id::new(self.id_ctr);
                    self.popup.replace(new_id);

                    let mut popup_settings = self.applet_helper.get_popup_settings(
                        window::Id::new(0),
                        new_id,
                        None,
                        None,
                        None,
                    );

                    popup_settings.positioner.size_limits = Limits::NONE
                        .min_height(1)
                        .min_width(1)
                        .max_height(800)
                        .max_width(400);
                    let tx = self.bluer_sender.as_ref().cloned();
                    return Command::batch(vec![
                        Command::perform(
                            async {
                                if let Some(tx) = tx {
                                    let _ = tx.send(BluerRequest::StateUpdate).await;
                                }
                            },
                            |_| Message::Ignore,
                        ),
                        get_popup(popup_settings),
                    ]);
                }
            }
            Message::Errored(_) => todo!(),
            Message::Ignore => {}
            Message::ToggleVisibleDevices(enabled) => {
                self.show_visible_devices = enabled;
            }
            Message::BluetoothEvent(e) => match e {
                BluerEvent::RequestResponse {
                    req,
                    state,
                    err_msg,
                } => {
                    if let Some(err_msg) = err_msg {
                        eprintln!("bluetooth request error: {}", err_msg);
                    }
                    self.bluer_state = state;
                    // TODO special handling for some requests
                    match req {
                        BluerRequest::StateUpdate
                            if self.popup.is_some() && self.bluer_sender.is_some() =>
                        {
                            let tx = self.bluer_sender.as_ref().cloned().unwrap();
                            return Command::perform(
                                async move {
                                    // sleep for a bit before requesting state update again
                                    tokio::time::sleep(Duration::from_millis(3000)).await;
                                    let _ = tx.send(BluerRequest::StateUpdate).await;
                                },
                                |_| Message::Ignore,
                            );
                        }
                        _ => {}
                    };
                }
                BluerEvent::Init { sender, state } => {
                    self.bluer_sender.replace(sender);
                    self.bluer_state = state;
                }
                BluerEvent::DevicesChanged { state } => {
                    self.bluer_state = state;
                }
                BluerEvent::Finished => {
                    // TODO should this exit with an error causing a restart?
                    eprintln!("bluetooth subscription finished. exiting...");
                    std::process::exit(0);
                }
                // TODO handle agent events
                BluerEvent::AgentEvent(event) => match event {
                    crate::bluetooth::BluerAgentEvent::DisplayPinCode(_d, _code) => {
                        // dbg!((d.name, code));
                    }
                    crate::bluetooth::BluerAgentEvent::DisplayPasskey(_d, _code) => {
                        // dbg!((d.name, code));
                    }
                    crate::bluetooth::BluerAgentEvent::RequestPinCode(_d) => {
                        // TODO anything to be done here?
                        // dbg!("request pin code", d.name);
                    }
                    crate::bluetooth::BluerAgentEvent::RequestPasskey(_d) => {
                        // TODO anything to be done here?
                        // dbg!("request passkey", d.name);
                    }
                    crate::bluetooth::BluerAgentEvent::RequestConfirmation(d, code, tx) => {
                        // dbg!("request confirmation", &d.name, &code);
                        self.request_confirmation.replace((d, code, tx));
                        // let _ = tx.send(false);
                    }
                    crate::bluetooth::BluerAgentEvent::RequestDeviceAuthorization(_d, _tx) => {
                        // TODO anything to be done here?
                        // dbg!("request device authorization", d.name);
                        // let_ = tx.send(false);
                    }
                    crate::bluetooth::BluerAgentEvent::RequestServiceAuthorization(
                        _d,
                        _service,
                        _tx,
                    ) => {
                        // my headphones seem to always request this
                        // doesn't seem to be defined in the UX mockups
                        // dbg!(
                        //     "request service authorization",
                        //     d.name,
                        //     bluer::id::Service::try_from(service)
                        //         .map(|s| s.to_string())
                        //         .unwrap_or_else(|_| "unknown".to_string())
                        // );
                    }
                },
            },
            Message::Request(r) => {
                match &r {
                    BluerRequest::SetBluetoothEnabled(enabled) => {
                        self.bluer_state.bluetooth_enabled = *enabled;
                        if !*enabled {
                            self.bluer_state = BluerState::default();
                        }
                    }
                    BluerRequest::ConnectDevice(add) => {
                        self.bluer_state
                            .devices
                            .iter_mut()
                            .find(|d| d.address == *add)
                            .map(|d| {
                                d.status = BluerDeviceStatus::Connecting;
                            });
                    }
                    BluerRequest::DisconnectDevice(add) => {
                        self.bluer_state
                            .devices
                            .iter_mut()
                            .find(|d| d.address == *add)
                            .map(|d| {
                                d.status = BluerDeviceStatus::Disconnecting;
                            });
                    }
                    BluerRequest::PairDevice(add) => {
                        self.bluer_state
                            .devices
                            .iter_mut()
                            .find(|d| d.address == *add)
                            .map(|d| {
                                d.status = BluerDeviceStatus::Pairing;
                            });
                    }
                    _ => {} // TODO
                }
                if let Some(tx) = self.bluer_sender.as_mut().cloned() {
                    return Command::perform(
                        async move {
                            let _ = tx.send(r).await;
                        },
                        |_| Message::Ignore, // Error handling
                    );
                }
            }
            Message::Cancel => {
                if let Some((_, _, tx)) = self.request_confirmation.take() {
                    return Command::perform(
                        async move {
                            let _ = tx.send(false).await;
                        },
                        |_| Message::Ignore,
                    );
                }
            }
            Message::Confirm => {
                if let Some((_, _, tx)) = self.request_confirmation.take() {
                    return Command::perform(
                        async move {
                            let _ = tx.send(true).await;
                        },
                        |_| Message::Ignore,
                    );
                }
            }
        }
        Command::none()
    }
    fn view(&self, id: SurfaceIdWrapper) -> Element<Message> {
        let button_style = Button::Custom {
            active: |t| iced_style::button::Appearance {
                border_radius: BorderRadius::from(0.0),
                ..t.active(&Button::Text)
            },
            hover: |t| iced_style::button::Appearance {
                border_radius: BorderRadius::from(0.0),
                ..t.hovered(&Button::Text)
            },
        };
        match id {
            SurfaceIdWrapper::LayerSurface(_) => unimplemented!(),
            SurfaceIdWrapper::Window(_) => self
                .applet_helper
                .icon_button(&self.icon_name)
                .on_press(Message::TogglePopup)
                .into(),
            SurfaceIdWrapper::Popup(_) => {
                let mut known_bluetooth = column![];
                for dev in self.bluer_state.devices.iter().filter(|d| {
                    !self
                        .request_confirmation
                        .as_ref()
                        .map_or(false, |(dev, _, _)| d.address == dev.address)
                }) {
                    let mut row = row![
                        icon(dev.icon.as_str(), 16).style(Svg::Symbolic),
                        text(dev.name.clone())
                            .size(14)
                            .horizontal_alignment(Horizontal::Left)
                            .vertical_alignment(Vertical::Center)
                            .width(Length::Fill)
                    ]
                    .align_items(Alignment::Center)
                    .spacing(12);

                    match &dev.status {
                        BluerDeviceStatus::Connected => {
                            row = row.push(
                                text(fl!("connected"))
                                    .size(14)
                                    .horizontal_alignment(Horizontal::Right)
                                    .vertical_alignment(Vertical::Center),
                            );
                        }
                        BluerDeviceStatus::Paired => {}
                        BluerDeviceStatus::Connecting | BluerDeviceStatus::Disconnecting => {
                            row = row.push(
                                icon("process-working-symbolic", 24)
                                    .style(Svg::Symbolic)
                                    .width(Length::Units(24))
                                    .height(Length::Units(24)),
                            );
                        }
                        BluerDeviceStatus::Disconnected | BluerDeviceStatus::Pairing => continue,
                    };

                    known_bluetooth = known_bluetooth.push(
                        button(APPLET_BUTTON_THEME)
                            .custom(vec![row.into()])
                            .style(APPLET_BUTTON_THEME)
                            .on_press(match dev.status {
                                BluerDeviceStatus::Connected => {
                                    Message::Request(BluerRequest::DisconnectDevice(dev.address))
                                }
                                BluerDeviceStatus::Disconnected => {
                                    Message::Request(BluerRequest::PairDevice(dev.address))
                                }
                                BluerDeviceStatus::Paired => {
                                    Message::Request(BluerRequest::ConnectDevice(dev.address))
                                }
                                BluerDeviceStatus::Connecting => {
                                    Message::Request(BluerRequest::CancelConnect(dev.address))
                                }
                                BluerDeviceStatus::Disconnecting => Message::Ignore, // Start connecting?
                                BluerDeviceStatus::Pairing => Message::Ignore, // Cancel pairing?
                            })
                            .width(Length::Fill),
                    );
                }

                let mut content = column![
                    column![
                        toggler(fl!("bluetooth"), self.bluer_state.bluetooth_enabled, |m| {
                            Message::Request(BluerRequest::SetBluetoothEnabled(m))
                        },)
                        .text_size(14)
                        .width(Length::Fill),
                        // these are not in the UX mockup, but they are useful imo
                        toggler(fl!("discoverable"), self.bluer_state.discoverable, |m| {
                            Message::Request(BluerRequest::SetDiscoverable(m))
                        },)
                        .text_size(14)
                        .width(Length::Fill),
                        toggler(fl!("pairable"), self.bluer_state.pairable, |m| {
                            Message::Request(BluerRequest::SetPairable(m))
                        },)
                        .text_size(14)
                        .width(Length::Fill)
                    ]
                    .spacing(8)
                    .padding([0, 12]),
                    divider::horizontal::light(),
                    known_bluetooth,
                ]
                .align_items(Alignment::Center)
                .spacing(8)
                .padding([8, 0]);
                let dropdown_icon = if self.show_visible_devices {
                    "go-down-symbolic"
                } else {
                    "go-next-symbolic"
                };
                let available_connections_btn = button(Button::Secondary)
                    .custom(
                        vec![
                            text(fl!("other-devices"))
                                .size(14)
                                .width(Length::Fill)
                                .height(Length::Units(24))
                                .vertical_alignment(Vertical::Center)
                                .into(),
                            container(
                                icon(dropdown_icon, 14)
                                    .style(Svg::Symbolic)
                                    .width(Length::Units(14))
                                    .height(Length::Units(14)),
                            )
                            .align_x(Horizontal::Center)
                            .align_y(Vertical::Center)
                            .width(Length::Units(24))
                            .height(Length::Units(24))
                            .into(),
                        ]
                        .into(),
                    )
                    .padding([8, 24])
                    .style(button_style.clone())
                    .on_press(Message::ToggleVisibleDevices(!self.show_visible_devices));
                content = content.push(available_connections_btn);
                let mut list_column: Vec<Element<'_, Message>> =
                    Vec::with_capacity(self.bluer_state.devices.len());

                if let Some((device, pin, _)) = self.request_confirmation.as_ref() {
                    let row = column![
                        icon(device.icon.as_str(), 16).style(Svg::Symbolic),
                        text(&device.name)
                            .horizontal_alignment(Horizontal::Left)
                            .vertical_alignment(Vertical::Center)
                            .width(Length::Fill),
                        text(fl!(
                            "confirm-pin",
                            HashMap::from_iter(vec![("deviceName", device.name.clone())])
                        ))
                        .horizontal_alignment(Horizontal::Left)
                        .vertical_alignment(Vertical::Center)
                        .width(Length::Fill)
                        .size(14),
                        text(pin)
                            .horizontal_alignment(Horizontal::Center)
                            .vertical_alignment(Vertical::Center)
                            .width(Length::Fill)
                            .size(32),
                        row![
                            button(Button::Secondary)
                                .custom(
                                    vec![text(fl!("cancel"))
                                        .size(14)
                                        .width(Length::Fill)
                                        .height(Length::Units(24))
                                        .vertical_alignment(Vertical::Center)
                                        .into(),]
                                    .into(),
                                )
                                .padding([8, 24])
                                .style(button_style.clone())
                                .on_press(Message::Cancel)
                                .width(Length::Fill),
                            button(Button::Secondary)
                                .custom(
                                    vec![text(fl!("confirm"))
                                        .size(14)
                                        .width(Length::Fill)
                                        .height(Length::Units(24))
                                        .vertical_alignment(Vertical::Center)
                                        .into(),]
                                    .into(),
                                )
                                .padding([8, 24])
                                .style(button_style.clone())
                                .on_press(Message::Confirm)
                                .width(Length::Fill),
                        ]
                    ]
                    .padding([0, 24])
                    .spacing(12);
                    list_column.push(row.into());
                }
                let mut visible_devices_count = 0;
                if self.show_visible_devices {
                    if self.bluer_state.bluetooth_enabled {
                        let mut visible_devices = column![];
                        for dev in self.bluer_state.devices.iter().filter(|d| {
                            matches!(
                                d.status,
                                BluerDeviceStatus::Disconnected | BluerDeviceStatus::Pairing
                            ) && !self
                                .request_confirmation
                                .as_ref()
                                .map_or(false, |(dev, _, _)| d.address == dev.address)
                        }) {
                            let row = row![
                                icon(dev.icon.as_str(), 16).style(Svg::Symbolic),
                                text(dev.name.clone())
                                    .horizontal_alignment(Horizontal::Left)
                                    .size(14),
                            ]
                            .width(Length::Fill)
                            .align_items(Alignment::Center)
                            .spacing(12);
                            visible_devices = visible_devices.push(
                                button(APPLET_BUTTON_THEME)
                                    .custom(vec![row.width(Length::Fill).into()])
                                    .on_press(Message::Request(BluerRequest::PairDevice(
                                        dev.address.clone(),
                                    )))
                                    .width(Length::Fill),
                            );
                            visible_devices_count += 1;
                        }
                        list_column.push(visible_devices.into());
                    }
                }
                let item_counter = visible_devices_count
                    // request confirmation is pretty big
                    + if self.request_confirmation.is_some() {
                        5
                    } else {
                        0
                    };

                if item_counter > 10 {
                    content = content.push(
                        scrollable(Column::with_children(list_column)).height(Length::Units(300)),
                    );
                } else {
                    content = content.push(Column::with_children(list_column));
                }
                self.applet_helper.popup_container(content).into()
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        bluetooth_subscription(0).map(|e| Message::BluetoothEvent(e.1))
    }

    fn theme(&self) -> Theme {
        self.theme
    }

    fn close_requested(&self, _id: SurfaceIdWrapper) -> Self::Message {
        Message::Ignore
    }

    fn style(&self) -> <Self::Theme as application::StyleSheet>::Style {
        <Self::Theme as application::StyleSheet>::Style::Custom(|theme| application::Appearance {
            background_color: Color::from_rgba(0.0, 0.0, 0.0, 0.0),
            text_color: theme.cosmic().on_bg_color().into(),
        })
    }
}
