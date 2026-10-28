//! The Update menu's dialogs: "Install Another Version…" (every release on
//! GitHub, plus the one saved previous version) and the download progress
//! shown while a release is fetched before the restart (`xpr_update`).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

use xpr_core::consts;
use xpr_ui_kit::widgets;
use xpr_update::{PendingSwap, Release};

use super::{action_button, body_text, enter_pressed, escape_pressed, fit_h, footer, help, list_well, modal, section, small_button, DialogCtx, DialogOutcome, MessageBox, MsgButtons, MsgTag, Tone};

pub enum DownloadMsg {
    Progress(u64, Option<u64>),
    Done(Result<PathBuf, String>),
}

pub enum UpdateDialog {
    Versions {
        rx: Option<Receiver<Result<Vec<Release>, String>>>,
        releases: Option<Result<Vec<Release>, String>>,
        previous: Option<String>,
    },
    Download {
        version: String,
        url: String,
        rx: Receiver<DownloadMsg>,
        done: u64,
        total: Option<u64>,
        cancel: Arc<AtomicBool>,
        error: Option<String>,
    },
}

/// The confirmation for installing `tag` from the version list.
pub fn install_prompt(tag: &str, url: &str) -> MessageBox {
    let text = format!(
        "Download and install {}? The program restarts once it is downloaded.\n\nThe version you are running now ({}) is kept, so you can go back to it from Update \u{25B8} Revert.",
        tag,
        consts::APP_VERSION
    );
    MessageBox::new(&format!("Install {}?", tag), &text, MsgButtons::YesNo, MsgTag::InstallRelease(tag.to_string(), url.to_string()))
}

/// The confirmation for rolling back to the saved previous version.
pub fn revert_prompt(previous: &str) -> MessageBox {
    let text = format!(
        "Go back to {}? The program restarts to switch.\n\nThe version you are running now ({}) is kept in its place, so you can switch back the same way.",
        previous,
        consts::APP_VERSION
    );
    MessageBox::new(&format!("Revert to {}?", previous), &text, MsgButtons::YesNo, MsgTag::RevertToPrevious(previous.to_string()))
}

fn mb(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1e6)
}

impl UpdateDialog {
    /// The version list; `previous` is the saved roll-back version.
    pub fn versions(previous: Option<String>) -> UpdateDialog {
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(xpr_update::list_releases());
        });
        UpdateDialog::Versions { rx: Some(rx), releases: None, previous }
    }

    /// Start downloading `version` from `url` into the staging folder.
    pub fn download(version: String, url: String) -> UpdateDialog {
        let (tx, rx) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let thread_cancel = cancel.clone();
        let thread_url = url.clone();
        std::thread::spawn(move || {
            let ptx = tx.clone();
            let mut progress = move |done: u64, total: Option<u64>| {
                let _ = ptx.send(DownloadMsg::Progress(done, total));
            };
            let result = xpr_update::download_release(&thread_url, &xpr_update::staging_dir(), &thread_cancel, &mut progress);
            if let Err(e) = &result {
                log::error!("update download failed: {}", e);
            }
            let _ = tx.send(DownloadMsg::Done(result));
        });
        log::info!("downloading {} from {}", version, url);
        UpdateDialog::Download { version, url, rx, done: 0, total: None, cancel, error: None }
    }

    pub fn ui(&mut self, ctx: &egui::Context, d: &mut DialogCtx, message: &mut Option<MessageBox>) -> (bool, Option<DialogOutcome>) {
        match self {
            UpdateDialog::Versions { .. } => (self.versions_ui(ctx, d, message), None),
            UpdateDialog::Download { .. } => self.download_ui(ctx, d),
        }
    }

    fn versions_ui(&mut self, ctx: &egui::Context, d: &mut DialogCtx, message: &mut Option<MessageBox>) -> bool {
        let UpdateDialog::Versions { rx, releases, previous } = self else { return true };
        if let Some(r) = rx {
            if let Ok(list) = r.try_recv() {
                *releases = Some(list);
                *rx = None;
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(200));
            }
        }
        let theme = d.theme;
        let possible = xpr_update::is_upgrade_possible();
        let mut close = false;
        modal(ctx, theme, "xpr_update_versions", "Install Another Version", 540.0, |ui| {
            section(ui, theme, "Saved previous version", None);
            match previous.as_deref() {
                Some(p) => {
                    ui.horizontal(|ui| {
                        ui.set_width(ui.available_width());
                        widgets::label_font(ui, p, theme.body_bold(), theme.text_strong());
                        widgets::label_font(ui, "kept from your last update", theme.body(), theme.secondary);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if small_button(ui, theme, &format!("Revert to {}", p), possible).clicked() {
                                *message = Some(revert_prompt(p));
                            }
                        });
                    });
                }
                None => help(ui, theme, "None yet. When you update, the version you were running is kept here (one copy) so you can go back to it."),
            }

            let installed = format!("you have {}", consts::APP_VERSION);
            section(ui, theme, "Releases on GitHub", Some(&installed));
            match releases {
                None => help(ui, theme, "Fetching releases\u{2026}"),
                Some(Err(e)) => help(ui, theme, &format!("Could not fetch the releases: {}", e)),
                Some(Ok(list)) if list.is_empty() => help(ui, theme, "No releases found for this platform."),
                Some(Ok(list)) => {
                    list_well(ui, theme, fit_h(ui, 320.0, 280.0), |ui| {
                        for r in list.iter() {
                            ui.horizontal(|ui| {
                                ui.set_min_height(30.0);
                                ui.set_width(ui.available_width());
                                ui.add_space(10.0);
                                widgets::label_font(ui, &r.tag, theme.body_bold(), theme.text_strong());
                                let detail = if r.asset_size > 0 { format!("{}  \u{00B7}  {}", r.published, mb(r.asset_size)) } else { r.published.clone() };
                                widgets::label_font(ui, detail, theme.body(), theme.secondary);
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.add_space(6.0);
                                    if xpr_update::same_version(&r.tag, consts::APP_VERSION) {
                                        widgets::label_font(ui, "Installed", theme.body(), theme.secondary);
                                    } else {
                                        let newer = xpr_update::is_upgrade_needed(Some(&r.tag), consts::APP_VERSION);
                                        if small_button(ui, theme, if newer { "Update" } else { "Install" }, possible).clicked() {
                                            *message = Some(install_prompt(&r.tag, &r.asset_url));
                                        }
                                    }
                                });
                            });
                        }
                    });
                }
            }
            if !possible {
                help(ui, theme, "Automatic updates are not supported for this installation: only the installed Windows or macOS program can update itself.");
            }
            footer(ui, theme, |ui| {
                if action_button(ui, theme, "Close", Tone::Primary, true).clicked() || escape_pressed(ui) {
                    close = true;
                }
            });
        });
        close
    }

    fn download_ui(&mut self, ctx: &egui::Context, d: &mut DialogCtx) -> (bool, Option<DialogOutcome>) {
        let UpdateDialog::Download { version, url, rx, done, total, cancel, error } = self else { return (true, None) };
        let mut finished = None;
        while let Ok(msg) = rx.try_recv() {
            match msg {
                DownloadMsg::Progress(n, t) => {
                    *done = n;
                    *total = t;
                }
                DownloadMsg::Done(r) => finished = Some(r),
            }
        }
        match finished {
            Some(Ok(app)) => {
                let swap = PendingSwap { new_app: app, to_version: version.clone(), revert: false };
                return (true, Some(DialogOutcome::SwapReady(swap)));
            }
            Some(Err(e)) => *error = Some(e),
            None => {}
        }
        if error.is_none() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        let theme = d.theme;
        let mut close = false;
        let mut retry = false;
        modal(ctx, theme, "xpr_update_download", &format!("Updating to {}", version), 460.0, |ui| {
            if let Some(e) = error.as_deref() {
                body_text(ui, theme, &format!("The download failed:\n{}", e));
                help(ui, theme, "Nothing was changed; you are still on the version you had.");
                footer(ui, theme, |ui| {
                    if action_button(ui, theme, "Retry", Tone::Primary, true).clicked() || enter_pressed(ui) {
                        retry = true;
                    }
                    if action_button(ui, theme, "Close", Tone::Secondary, true).clicked() || escape_pressed(ui) {
                        close = true;
                    }
                });
                return;
            }
            let extracting = total.is_some_and(|t| t > 0 && *done >= t);
            body_text(ui, theme, &if extracting { "Unpacking\u{2026}".to_string() } else { format!("Downloading {}\u{2026}", version) });
            ui.add_space(6.0);
            let frac = match total {
                Some(t) if *t > 0 => (*done as f32 / *t as f32).clamp(0.0, 1.0),
                _ => 0.0,
            };
            ui.add(egui::ProgressBar::new(frac).desired_width(ui.available_width()).fill(theme.accent));
            ui.add_space(4.0);
            let amount = match total {
                Some(t) => format!("{} of {}", mb(*done), mb(*t)),
                None => mb(*done),
            };
            help(ui, theme, &amount);
            help(ui, theme, "When it is done the program closes, installs the new version and restarts (you are asked about unsaved changes first).");
            footer(ui, theme, |ui| {
                if action_button(ui, theme, "Cancel", Tone::Secondary, true).clicked() || escape_pressed(ui) {
                    cancel.store(true, Ordering::Relaxed);
                    close = true;
                }
            });
        });
        if retry {
            *self = UpdateDialog::download(version.clone(), url.clone());
        }
        (close, None)
    }
}
