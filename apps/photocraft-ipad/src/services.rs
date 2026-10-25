//! Native document I/O, preferences, and recovery inside the app container.
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use photocraft_codecs::{ChannelLayout, EncodeOptions, Image};
use photocraft_format::RecoveryStore;
use photocraft_ui_egui::{FileDialogAnswer, FileStamp, Recoverable, Services};

use crate::files::{Documents, Picker};

fn guarded<T>(operation: &str, run: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)).map_err(|_| format!("{operation} could not finish safely; your document remains open"))?
}

pub fn make(documents: Documents, picker: Rc<RefCell<Picker>>) -> Services {
    let read_root = documents.clone();
    let write_root = documents.clone();
    let private = documents.path().join(".photocraft-ipad");
    let prefs = private.join("preferences.json");
    let load_prefs = prefs.clone();
    let store = Rc::new(RefCell::new(RecoveryStore::new(private.join("recovery"))));
    let (s1, s2, s3, s4, s5) = (store.clone(), store.clone(), store.clone(), store.clone(), store);
    let recovery_dir = private.join("recovery");
    Services {
        import: Some(Box::new(|name, bytes, options| {
            guarded("Open", || photocraft_io::import_with_options(name, bytes, options).map(|r| (r.document, r.warnings)).map_err(|e| e.to_string()))
        })),
        export: Some(Box::new(|document, name, settings| {
            guarded("Export", || photocraft_io::export(document, name, &settings.io_options()).map(|r| (r.bytes, r.warnings)).map_err(|e| e.to_string()))
        })),
        file_dialog: Some(Box::new(move |request, _, reply| match picker.try_borrow_mut() {
            Ok(mut picker) => picker.ask(request, reply),
            Err(_) => reply.send(Some(FileDialogAnswer::Unavailable("The file chooser is busy; try again".into()))),
        })),
        write: Some(Box::new(move |path, bytes| write_root.write(Path::new(path), bytes))),
        file_stamp: Some(Box::new(move |path| {
            let path = read_root.existing(Path::new(path)).ok()?;
            let metadata = std::fs::metadata(path).ok()?;
            Some(FileStamp { len: metadata.len(), modified: metadata.modified().ok() })
        })),
        encode_png: Some(Box::new(|width, height, bytes| {
            let image = Image::from_u8(width, height, ChannelLayout::Rgba, bytes.to_vec()).map_err(|e| e.to_string())?;
            photocraft_codecs::encode(&image, photocraft_codecs::Format::Png, &EncodeOptions::default()).map_err(|e| e.to_string())
        })),
        load_prefs: Some(Box::new(move || std::fs::read_to_string(&load_prefs).ok())),
        save_prefs: Some(Box::new(move |text| {
            std::fs::create_dir_all(&private).map_err(|e| e.to_string())?;
            photocraft_format::atomic_write(&prefs, text.as_bytes()).map_err(|e| e.to_string())
        })),
        autosave: Some(Box::new(move |document, revision, path| {
            s1.try_borrow_mut().map_err(|_| "Recovery store is busy".to_owned())?.autosave_checked(document, revision, path.map(str::to_owned))
        })),
        autosave_results: Some(Box::new(move || s2.try_borrow_mut().map(|mut s| s.take_completed()).unwrap_or_default())),
        flush_autosaves: Some(Box::new(move || s3.try_borrow_mut().map(|mut s| s.flush()).unwrap_or_default())),
        discard_autosave: Some(Box::new(move |id| {
            if let Ok(mut store) = s4.try_borrow_mut() {
                let _ = store.discard(id);
            }
        })),
        recover: Some(Box::new(move || {
            photocraft_format::list_recovery(&recovery_dir)
                .into_iter()
                .map(|entry| {
                    let key = entry.info.key.clone();
                    let name = entry.info.document_name.clone();
                    let path = entry.info.original_path.clone();
                    Recoverable { key, name, path, load: Box::new(move || photocraft_format::recover(&entry).map_err(|e| e.to_string())) }
                })
                .collect()
        })),
        adopt_autosave: Some(Box::new(move |id, key| {
            if let Ok(mut store) = s5.try_borrow_mut() {
                store.adopt(id, key);
            }
        })),
        same_file: Some(Box::new(|a, b| Path::new(a).canonicalize().ok().zip(Path::new(b).canonicalize().ok()).is_some_and(|(a, b)| a == b))),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ipad_services_open_and_export_real_images() {
        let temp = tempfile::tempdir().unwrap();
        let documents = Documents::new(temp.path()).unwrap();
        let picker = Rc::new(RefCell::new(Picker::new(documents.clone())));
        let mut services = make(documents.clone(), picker);
        let image = Image::from_u8(2, 1, ChannelLayout::Rgba, vec![255, 0, 0, 255, 0, 255, 0, 255]).unwrap();
        let png = photocraft_codecs::encode(&image, photocraft_codecs::Format::Png, &EncodeOptions::default()).unwrap();
        let (document, _) = services.import.as_ref().unwrap()("test.png", &png, &Default::default()).unwrap();
        let (psd, _) = services.export.as_ref().unwrap()(&document, "edited.psd", &Default::default()).unwrap();
        assert!(psd.starts_with(b"8BPS"));
        let saved = documents.path().join("edited.psd");
        services.write.as_mut().unwrap()(saved.to_str().unwrap(), &psd).unwrap();
        let (reopened, _) = services.import.as_ref().unwrap()("edited.psd", &std::fs::read(saved).unwrap(), &Default::default()).unwrap();
        assert_eq!(reopened.size, document.size);
    }
    #[test]
    fn corrupt_images_return_an_error() {
        let temp = tempfile::tempdir().unwrap();
        let documents = Documents::new(temp.path()).unwrap();
        let services = make(documents.clone(), Rc::new(RefCell::new(Picker::new(documents))));
        assert!(services.import.as_ref().unwrap()("broken.psd", b"not a PSD", &Default::default()).is_err());
    }
}
