//! `NotificationPopup` (bottom-right green toast with a 200 ms fade and an
//! optional "Open Folder" button) and `AutoClearingLabel`.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui::{Align2, Color32, CornerRadius, Pos2, Sense, Stroke, Vec2};

use crate::theme::Theme;

const FADE: Duration = Duration::from_millis(200);

#[derive(Clone, Debug)]
pub struct Toast {
    message: String,
    folder_path: Option<PathBuf>,
    shown_at: Instant,
    duration: Duration,
    hide_requested_at: Option<Instant>,
}

/// The one toast the window shows (a new notification replaces it).
#[derive(Clone, Debug, Default)]
pub struct ToastHost {
    toast: Option<Toast>,
}

impl ToastHost {
    /// `show_notification(message, duration, folder_path)`
    pub fn show(&mut self, message: impl Into<String>, duration_ms: u64, folder_path: Option<PathBuf>) {
        self.toast = Some(Toast {
            message: message.into(),
            folder_path,
            shown_at: Instant::now(),
            duration: Duration::from_millis(duration_ms),
            hide_requested_at: None,
        });
    }

    pub fn hide(&mut self) {
        self.toast = None;
    }

    /// Draw the toast (if any) in the bottom-right corner of the viewport.
    /// Returns a folder path when the user clicked "Open Folder".
    pub fn ui(&mut self, ctx: &egui::Context, theme: &Theme) -> Option<PathBuf> {
        let Some(t) = self.toast.as_mut() else { return None };
        let now = Instant::now();
        let since = now.duration_since(t.shown_at);
        let hide_at = t.hide_requested_at.unwrap_or(t.shown_at + t.duration);
        if now >= hide_at + FADE {
            self.toast = None;
            return None;
        }
        let opacity = if since < FADE {
            since.as_secs_f32() / FADE.as_secs_f32()
        } else if now >= hide_at {
            1.0 - now.duration_since(hide_at).as_secs_f32() / FADE.as_secs_f32()
        } else {
            1.0
        }
        .clamp(0.0, 1.0);
        ctx.request_repaint_after(Duration::from_millis(16));

        let mut open_folder: Option<PathBuf> = None;
        let mut dismiss = false;
        let alpha = |c: Color32| Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (opacity * 255.0) as u8);
        egui::Area::new(egui::Id::new("xpr_toast"))
            .order(egui::Order::Tooltip)
            .anchor(Align2::RIGHT_BOTTOM, Vec2::new(-20.0, -20.0))
            .interactable(true)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(alpha(Color32::from_rgb(0x4c, 0xaf, 0x50)))
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(egui::Margin { left: 12, right: 12, top: 8, bottom: 8 })
                    .show(ui, |ui| {
                        ui.set_max_width(400.0);
                        ui.vertical(|ui| {
                            ui.add(
                                egui::Label::new(egui::RichText::new(t.message.clone()).font(theme.font_bold(11.0)).color(alpha(Color32::WHITE)))
                                    .wrap(),
                            );
                            if t.folder_path.is_some() {
                                let font = theme.body();
                                let galley = ui.fonts_mut(|f| f.layout_no_wrap("Open Folder".to_string(), font, Color32::PLACEHOLDER));
                                let (rect, resp) = ui.allocate_exact_size(galley.size() + Vec2::new(20.0, 8.0), Sense::click());
                                let fill = if resp.hovered() { Color32::from_rgb(0x81, 0xc7, 0x84) } else { Color32::from_rgb(0x66, 0xbb, 0x6a) };
                                ui.painter().rect(rect, CornerRadius::same(3), alpha(fill), Stroke::new(1.0_f32, alpha(Color32::from_rgb(0x81, 0xc7, 0x84))), egui::StrokeKind::Inside);
                                ui.painter().galley(Pos2::new(rect.center().x - galley.size().x / 2.0, rect.center().y - galley.size().y / 2.0), galley, alpha(Color32::WHITE));
                                if resp.clicked() {
                                    open_folder = t.folder_path.clone();
                                    dismiss = true;
                                }
                            }
                        });
                    });
            });
        if dismiss {
            self.toast = None;
        }
        open_folder
    }
}

/// `AutoClearingLabel(clear_timeout=3000)`: a message that disappears on its own.
#[derive(Clone, Debug, Default)]
pub struct AutoClearingLabel {
    text: String,
    set_at: Option<Instant>,
    timeout: Duration,
}

impl AutoClearingLabel {
    pub fn new(timeout_ms: u64) -> AutoClearingLabel {
        AutoClearingLabel { text: String::new(), set_at: None, timeout: Duration::from_millis(timeout_ms) }
    }

    pub fn set_message(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.set_at = if self.text.is_empty() { None } else { Some(Instant::now()) };
    }

    pub fn is_visible(&self) -> bool {
        match self.set_at {
            Some(t) => Instant::now().duration_since(t) < self.timeout,
            None => false,
        }
    }

    /// Draw the label if it is visible (nothing is allocated otherwise).
    pub fn ui(&mut self, ui: &mut egui::Ui, theme: &Theme) {
        if let Some(t) = self.set_at {
            let elapsed = Instant::now().duration_since(t);
            if elapsed >= self.timeout {
                self.text.clear();
                self.set_at = None;
                return;
            }
            ui.ctx().request_repaint_after(self.timeout - elapsed);
            ui.label(egui::RichText::new(self.text.clone()).font(theme.body()).color(theme.text));
        }
    }
}
