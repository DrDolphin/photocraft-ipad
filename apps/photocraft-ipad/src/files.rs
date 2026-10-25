//! Native file chooser restricted to the application's Files-visible Documents.
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use photocraft_ui_egui::{FileDialogAnswer, FileDialogReply, FileDialogRequest};

#[derive(Clone)]
pub struct Documents(PathBuf);

fn path_text(path: &Path) -> Result<String, String> {
    path.to_str().map(str::to_owned).ok_or_else(|| "The file or folder name is not valid Unicode; rename it in Files".into())
}

impl Documents {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, String> {
        std::fs::create_dir_all(path.as_ref()).map_err(|e| format!("Cannot create Documents: {e}"))?;
        path.as_ref().canonicalize().map(Self).map_err(|e| e.to_string())
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn existing(&self, path: &Path) -> Result<PathBuf, String> {
        let canonical = path.canonicalize().map_err(|e| e.to_string())?;
        if canonical.starts_with(&self.0) { Ok(canonical) } else { Err("Choose a file in PhotoCraft's Documents folder".into()) }
    }

    pub fn destination(&self, directory: &Path, name: &str) -> Result<PathBuf, String> {
        let name = name.trim();
        if name.is_empty()
            || name.contains(['/', '\\', ':', '\0'])
            || name.chars().any(char::is_control)
            || !matches!(Path::new(name).components().collect::<Vec<_>>().as_slice(), [Component::Normal(_)])
        {
            return Err("Enter a file name without a folder or path".into());
        }
        let directory = self.existing(directory)?;
        if !directory.is_dir() {
            return Err("The selected location is not a folder".into());
        }
        let path = directory.join(name);
        if std::fs::symlink_metadata(&path).is_ok() {
            self.existing(&path)?;
            if path.is_dir() {
                return Err("A folder already has that name".into());
            }
        }
        Ok(path)
    }

    pub fn write(&self, path: &Path, bytes: &[u8]) -> Result<(), String> {
        let name = path.file_name().and_then(|name| name.to_str()).ok_or("The file name is not Unicode")?;
        let parent = path.parent().ok_or("The file has no folder")?;
        let destination = self.destination(parent, name)?;
        photocraft_format::atomic_write(&destination, bytes).map_err(|e| e.to_string())
    }
}

pub struct Picker {
    documents: Documents,
    directory: PathBuf,
    request: Option<FileDialogRequest>,
    reply: Option<FileDialogReply>,
    entries: Vec<(PathBuf, bool)>,
    selected: BTreeSet<PathBuf>,
    filename: String,
    error: Option<String>,
}

impl Picker {
    pub fn new(documents: Documents) -> Self {
        Self {
            directory: documents.path().to_path_buf(),
            documents,
            request: None,
            reply: None,
            entries: Vec::new(),
            selected: BTreeSet::new(),
            filename: String::new(),
            error: None,
        }
    }

    pub fn ask(&mut self, request: FileDialogRequest, reply: FileDialogReply) {
        if self.reply.is_some() {
            reply.send(Some(FileDialogAnswer::Unavailable("A file chooser is already open".into())));
            return;
        }
        self.filename = match &request {
            FileDialogRequest::Save { suggested } => Path::new(suggested).file_name().and_then(|n| n.to_str()).unwrap_or("Untitled.psd").to_owned(),
            _ => String::new(),
        };
        self.request = Some(request);
        self.reply = Some(reply);
        self.selected.clear();
        self.refresh();
    }

    fn refresh(&mut self) {
        self.error = None;
        self.entries.clear();
        let listing = match std::fs::read_dir(&self.directory) {
            Ok(entries) => entries,
            Err(e) => {
                self.error = Some(e.to_string());
                return;
            }
        };
        let extensions = match &self.request {
            Some(FileDialogRequest::Open { extensions, .. }) => extensions.as_ref(),
            _ => None,
        };
        for entry in listing.take(10_001) {
            if self.entries.len() >= 10_000 {
                self.error = Some("This folder has too many entries. Organize files into subfolders in Files.".into());
                break;
            }
            let Ok(entry) = entry else { continue };
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else { continue };
            if name.starts_with('.') {
                continue;
            }
            let Ok(path) = self.documents.existing(&entry.path()) else { continue };
            let directory = path.is_dir();
            let allowed = directory
                || extensions.is_none_or(|extensions| {
                    path.extension().and_then(|e| e.to_str()).is_some_and(|ext| extensions.iter().any(|e| e.eq_ignore_ascii_case(ext)))
                });
            if allowed {
                self.entries.push((path, directory));
            }
        }
        self.entries.sort_by(|(a, a_dir), (b, b_dir)| b_dir.cmp(a_dir).then_with(|| a.file_name().cmp(&b.file_name())));
    }

    fn finish(&mut self, answer: Option<FileDialogAnswer>) {
        self.request = None;
        self.selected.clear();
        if let Some(reply) = self.reply.take() {
            reply.send(answer);
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        let Some(request) = self.request.clone() else { return };
        let save = matches!(request, FileDialogRequest::Save { .. });
        let folder = matches!(request, FileDialogRequest::Folder { .. });
        let multiple = matches!(request, FileDialogRequest::Open { multiple: true, .. });
        let title = if save {
            "Save in PhotoCraft"
        } else if folder {
            "Choose a PhotoCraft folder"
        } else {
            "Open from PhotoCraft"
        };
        let mut open = true;
        egui::Window::new(title)
            .id(egui::Id::new("ipad_files"))
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .open(&mut open)
            .collapsible(false)
            .default_size([620.0, 460.0])
            .min_size([280.0, 220.0])
            .show(ctx, |ui| {
                ui.label("Copy files here using Files → On My iPad → PhotoCraft. Saved files appear there too.");
                ui.horizontal(|ui| {
                    if ui.add_enabled(self.directory != self.documents.path(), egui::Button::new("Up").min_size(egui::vec2(56.0, 44.0))).clicked()
                        && let Some(parent) = self.directory.parent().and_then(|p| self.documents.existing(p).ok())
                    {
                        self.directory = parent;
                        self.selected.clear();
                        self.refresh();
                    }
                    if ui.add_sized([84.0, 44.0], egui::Button::new("Refresh")).clicked() {
                        self.refresh();
                    }
                    ui.label(
                        self.directory
                            .strip_prefix(self.documents.path())
                            .ok()
                            .filter(|p| !p.as_os_str().is_empty())
                            .map_or_else(|| "Documents".to_owned(), |p| p.display().to_string()),
                    );
                });
                let mut enter = None;
                egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                    if self.entries.is_empty() {
                        ui.label("No matching files. Copy an image here in the Files app, then tap Refresh.");
                    }
                    for (path, directory) in &self.entries {
                        let Some(name) = path.file_name().and_then(|name| name.to_str()) else { continue };
                        if *directory {
                            if ui.add_sized([ui.available_width(), 44.0], egui::Button::new(format!("Folder · {name}"))).clicked() {
                                enter = Some(path.clone());
                            }
                        } else if !folder
                            && ui.add_sized([ui.available_width(), 44.0], egui::Button::new(name).selected(self.selected.contains(path))).clicked()
                        {
                            if save {
                                self.filename = name.to_owned();
                            } else if self.selected.contains(path) {
                                self.selected.remove(path);
                            } else {
                                if !multiple {
                                    self.selected.clear();
                                }
                                self.selected.insert(path.clone());
                            }
                        }
                    }
                });
                if let Some(path) = enter {
                    self.directory = path;
                    self.selected.clear();
                    self.refresh();
                }
                if save {
                    ui.label("File name (the extension chooses the format):");
                    ui.add_sized([ui.available_width(), 44.0], egui::TextEdit::singleline(&mut self.filename));
                }
                if let Some(error) = &self.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
                ui.horizontal(|ui| {
                    if ui.add_sized([100.0, 44.0], egui::Button::new("Cancel")).clicked() {
                        self.finish(None);
                    }
                    let enabled = save || folder || !self.selected.is_empty();
                    if ui
                        .add_enabled(
                            enabled,
                            egui::Button::new(if save {
                                "Save"
                            } else if folder {
                                "Choose folder"
                            } else {
                                "Open"
                            })
                            .min_size(egui::vec2(130.0, 44.0)),
                        )
                        .clicked()
                    {
                        let answer = if save {
                            self.documents.destination(&self.directory, &self.filename).and_then(|p| path_text(&p)).map(FileDialogAnswer::SaveTo)
                        } else if folder {
                            path_text(&self.directory).map(FileDialogAnswer::FolderTo)
                        } else {
                            self.selected.iter().map(|p| path_text(p)).collect::<Result<Vec<_>, _>>().map(FileDialogAnswer::Paths)
                        };
                        match answer {
                            Ok(answer) => self.finish(Some(answer)),
                            Err(error) => self.error = Some(error),
                        }
                    }
                });
            });
        if !open {
            self.finish(None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saves_are_atomic_and_restricted_to_documents() {
        let temp = tempfile::tempdir().unwrap();
        let docs = Documents::new(temp.path().join("Documents")).unwrap();
        let file = docs.destination(docs.path(), "drawing.psd").unwrap();
        docs.write(&file, b"first").unwrap();
        docs.write(&file, b"second").unwrap();
        assert_eq!(std::fs::read(file).unwrap(), b"second");
        assert!(docs.write(&temp.path().join("outside.psd"), b"no").is_err());
        for bad in ["", ".", "..", "../escape.png", "folder/file.png", "folder\\file.png", "bad\0.png", "a:b.png", "bad\n.png"] {
            assert!(docs.destination(docs.path(), bad).is_err(), "{bad:?}");
        }
    }
    #[test]
    fn nested_files_are_available_and_folders_cannot_be_overwritten() {
        let temp = tempfile::tempdir().unwrap();
        let docs = Documents::new(temp.path()).unwrap();
        std::fs::create_dir(docs.path().join("Sketches")).unwrap();
        let file = docs.destination(&docs.path().join("Sketches"), "été.png").unwrap();
        docs.write(&file, b"image").unwrap();
        assert!(docs.existing(&file).is_ok());
        assert!(docs.destination(docs.path(), "Sketches").is_err());
    }
    #[cfg(not(target_os = "ios"))]
    #[test]
    #[ignore = "Offscreen visual QA: writes portrait and landscape PNGs to PHOTOCRAFT_IPAD_QA_DIR"]
    fn render_native_file_chooser() {
        use std::cell::RefCell;
        use std::rc::Rc;
        let output = std::env::var_os("PHOTOCRAFT_IPAD_QA_DIR").map(PathBuf::from).expect("set PHOTOCRAFT_IPAD_QA_DIR");
        std::fs::create_dir_all(&output).unwrap();
        for (name, width, height) in [("landscape", 1180.0, 820.0), ("portrait", 820.0, 1180.0), ("compact", 768.0, 1024.0)] {
            let temp = tempfile::tempdir().unwrap();
            let docs = Documents::new(temp.path()).unwrap();
            std::fs::create_dir(docs.path().join("Sketches")).unwrap();
            std::fs::write(docs.path().join("Landscape.psd"), []).unwrap();
            std::fs::write(docs.path().join("Portrait.png"), []).unwrap();
            let picker = Rc::new(RefCell::new(Picker::new(docs.clone())));
            let mut harness = egui_kittest::Harness::builder().with_size([width, height]).wgpu().build_eframe(move |cc| {
                photocraft_ui_egui::PhotocraftApp::setup_context(&cc.egui_ctx, photocraft_ui_egui::theme::ThemeKind::Pro);
                let mut inner = photocraft_ui_egui::PhotocraftApp::new(photocraft_engine::Session::new(), crate::services::make(docs, picker.clone()));
                inner.custom_titlebar = false;
                if let Some(state) = cc.wgpu_render_state.clone() {
                    inner.set_wgpu(state);
                }
                crate::IpadApp { inner, picker, show_help: true }
            });
            harness.state_mut().show_help = false;
            let context = harness.ctx.clone();
            photocraft_ui_egui::menus::invoke(&mut harness.state_mut().inner, &context, "file.open", Default::default()).unwrap();
            harness.run_steps(6);
            assert!(harness.state().picker.try_borrow().unwrap().request.is_some(), "File → Open reaches the native chooser");
            let image = harness.render().unwrap();
            let pixels = photocraft_codecs::Image::from_u8(image.width(), image.height(), photocraft_codecs::ChannelLayout::Rgba, image.into_raw()).unwrap();
            let bytes = photocraft_codecs::encode(&pixels, photocraft_codecs::Format::Png, &Default::default()).unwrap();
            std::fs::write(output.join(format!("PhotoCraft-iPad-{name}.png")), bytes).unwrap();
        }
    }

    #[cfg(unix)]
    #[test]
    fn links_cannot_escape_the_container() {
        let temp = tempfile::tempdir().unwrap();
        let docs = Documents::new(temp.path().join("Documents")).unwrap();
        std::fs::write(temp.path().join("outside"), b"keep").unwrap();
        std::os::unix::fs::symlink(temp.path().join("outside"), docs.path().join("link")).unwrap();
        assert!(docs.existing(&docs.path().join("link")).is_err());
        assert!(docs.destination(docs.path(), "link").is_err());
    }
}
