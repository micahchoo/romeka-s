//! The pixel sizes of stored images, read from their headers and remembered.
//!
//! A canvas needs its image's size, and the media rows store none (`media.data`
//! is upstream's column, so this never writes it). The first request that needs
//! a file's size reads its header through the store; the size is kept in
//! memory by storage path for the life of the process. A stored file never
//! changes under its path, so nothing invalidates an entry.

use std::collections::HashMap;
use std::io::Cursor;
use std::sync::{Arc, Mutex};

use omeka_media::image_metadata;
use omeka_media::store::Store;

use super::manifest::Size;

/// How much of a file the first read takes. A JPEG's frame header follows its
/// metadata segments, which this covers for any file a camera writes.
const HEAD_BYTES: u64 = 256 * 1024;

/// The remembered sizes, shared by every request.
#[derive(Debug, Default)]
pub(crate) struct Dimensions {
    known: Mutex<HashMap<String, Size>>,
}

impl Dimensions {
    /// The size of the image at `storage_path`, `None` when the file is
    /// missing or its header names no size.
    pub(crate) async fn of(&self, store: &Arc<dyn Store>, storage_path: &str) -> Option<Size> {
        if let Some(size) = self.remembered(storage_path) {
            return Some(size);
        }
        let store = Arc::clone(store);
        let path = storage_path.to_string();
        let size = tokio::task::spawn_blocking(move || read_size(store.as_ref(), &path))
            .await
            .ok()??;
        if let Ok(mut known) = self.known.lock() {
            known.insert(storage_path.to_string(), size);
        }
        Some(size)
    }

    fn remembered(&self, storage_path: &str) -> Option<Size> {
        self.known.lock().ok()?.get(storage_path).copied()
    }
}

fn read_size(store: &dyn Store, storage_path: &str) -> Option<Size> {
    let head = store.read_range(storage_path, 0..HEAD_BYTES).ok()?;
    size_of(&head).or_else(|| size_of(&store.read(storage_path).ok()?))
}

fn size_of(bytes: &[u8]) -> Option<Size> {
    let tags = image_metadata(&mut Cursor::new(bytes));
    let width = tags.get("Width")?.parse().ok()?;
    let height = tags.get("Height")?.parse().ok()?;
    Some((width, height))
}
