//! A native iPadOS executable: the existing Rust editor, egui and wgpu/Metal.
//! Files are exchanged through the app's Documents directory, exposed in Files.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod files;
mod services;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use photocraft_engine::Session;
use photocraft_ui_egui::{PhotocraftApp, theme::ThemeKind};

fn documents() -> Result<PathBuf, String> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--documents" {
            return args.next().map(PathBuf::from).ok_or_else(|| "--documents needs a folder".into());
        }
    }
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).ok_or("The application has no home directory")?;
    Ok(PathBuf::from(home).join("Documents"))
}

fn main() -> eframe::Result {
    let root = documents().and_then(files::Documents::new).map_err(|error| eframe::Error::AppCreation(error.into()))?;
    let picker = Rc::new(RefCell::new(files::Picker::new(root.clone())));
    let ui_picker = picker.clone();
    let mut options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("PhotoCraft · iPad development build").with_inner_size([1180.0, 820.0]),
        renderer: eframe::Renderer::Wgpu,
        run_and_return: false,
        ..Default::default()
    };
    photocraft_ui_egui::gpu_canvas::use_adapter_limits(&mut options.wgpu_options.wgpu_setup);
    #[cfg(target_os = "ios")]
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut options.wgpu_options.wgpu_setup {
        setup.instance_descriptor.backends = eframe::wgpu::Backends::METAL;
    }
    eframe::run_native(
        "PhotoCraft iPad",
        options,
        Box::new(move |cc| {
            PhotocraftApp::setup_context(&cc.egui_ctx, ThemeKind::Pro);
            let mut inner = PhotocraftApp::new(Session::new(), services::make(root, picker));
            inner.custom_titlebar = false;
            if let Some(state) = cc.wgpu_render_state.clone() {
                inner.set_wgpu(state);
            }
            Ok(Box::new(IpadApp { inner, picker: ui_picker, show_help: true }))
        }),
    )
}

struct IpadApp {
    inner: PhotocraftApp,
    picker: Rc<RefCell<files::Picker>>,
    show_help: bool,
}

impl eframe::App for IpadApp {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.inner.logic(ctx, frame);
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.inner.ui(ui, frame);
        // PhotoCraft installs named fonts at startup; egui applies them on a later pass.
        // Its own editor waits for those fonts, and shell dialogs must wait as well.
        if ui.ctx().cumulative_pass_nr() < 2 {
            ui.ctx().request_repaint();
            return;
        }
        if let Ok(mut picker) = self.picker.try_borrow_mut() {
            picker.show(ui.ctx());
        }
        if self.show_help {
            egui::Window::new("PhotoCraft · native iPad preview")
                .collapsible(false).resizable(false).default_width(470.0)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .show(ui.ctx(), |ui| {
                    ui.heading("Your files are in the Files app");
                    ui.label("Copy a photo, PNG, JPEG or PSD into On My iPad → PhotoCraft, then use File → Open here. Saves and exports appear in the same folder.");
                    ui.add_space(10.0);
                    ui.label("This development build uses the native Rust editor and Metal. Its file chooser is limited to PhotoCraft's Documents folder. iCloud document pickers, Share/Open With, and full Apple Pencil behavior still need device integration and testing.");
                    ui.add_space(10.0);
                    ui.label("Save before switching apps. Recovery snapshots supplement saved files, but iPadOS can terminate a background app.");
                    if ui.add_sized([180.0, 44.0], egui::Button::new("Start editing")).clicked() {
                        self.show_help = false;
                    }
                });
        }
    }

    fn raw_input_hook(&mut self, ctx: &egui::Context, input: &mut egui::RawInput) {
        self.inner.raw_input_hook(ctx, input);
    }

    fn on_exit(&mut self) {
        self.inner.on_exit();
        if let Some(flush) = self.inner.services.flush_autosaves.as_mut() {
            for (_, _, result) in flush() {
                if let Err(error) = result {
                    log::warn!("Recovery snapshot: {error}");
                }
            }
        }
    }
}
