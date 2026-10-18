//! Test helpers: a migrated temp database, and one with the default install.

use std::path::PathBuf;

use sqlx::SqlitePool;
use tempfile::TempDir;

use omeka_db::ids::JobId;
use omeka_db::{migrate, open_file};
use serde_json::Value;

use crate::jobs::Job;

/// A migrated SQLite file that lives as long as the value.
pub struct TempDb {
    /// Kept (unread) so the directory outlives the database file.
    pub _directory: TempDir,
    pub path: PathBuf,
    pub pool: SqlitePool,
}

/// A fresh migrated database.
pub async fn temp_db() -> TempDb {
    let directory = tempfile::tempdir().expect("temp dir");
    let path = directory.path().join("romeka.db");
    let pool = open_file(&path).await.expect("open temp database");
    migrate(&pool).await.expect("migrate temp database");
    TempDb {
        _directory: directory,
        path,
        pool,
    }
}

/// A migrated database with the default vocabularies, template and admin user.
pub async fn seeded_db() -> TempDb {
    let database = temp_db().await;
    crate::commands::install::run(&crate::commands::install::InstallOptions {
        database: database.path.clone(),
        email: "admin@example.org".to_string(),
        name: "Admin".to_string(),
        password: "hunter2".to_string(),
        title: "Test".to_string(),
        time_zone: "UTC".to_string(),
        locale: "en".to_string(),
    })
    .await
    .expect("seed database");
    database
}

/// Compose a job row in `starting`, the way `Dispatcher::dispatch` composes one
/// before it starts a process. The tests run the row with `Runner::run`.
pub async fn compose(pool: &SqlitePool, job: &dyn Job, args: Value) -> JobId {
    crate::store::job_create(pool, job.class(), &args, None)
        .await
        .expect("compose job row")
}

/// A valid 8-bit RGB PNG of the given size, so a test can exercise the image
/// thumbnailer without adding an image encoder dependency to this crate.
///
/// The pixels form a colour ramp; the pixel values do not matter to the
/// thumbnailer, only that the bytes decode.
pub fn png_rgb(width: u32, height: u32) -> Vec<u8> {
    let mut raw = Vec::new();
    for y in 0..height {
        raw.push(0);
        for x in 0..width {
            raw.push((x % 256) as u8);
            raw.push((y % 256) as u8);
            raw.push(((x + y) % 256) as u8);
        }
    }
    let mut png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    append_png_chunk(&mut png, b"IHDR", &ihdr);
    append_png_chunk(&mut png, b"IDAT", &zlib_stored(&raw));
    append_png_chunk(&mut png, b"IEND", &[]);
    png
}

fn append_png_chunk(png: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    png.extend_from_slice(&(data.len() as u32).to_be_bytes());
    png.extend_from_slice(kind);
    png.extend_from_slice(data);
    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(kind);
    crc_input.extend_from_slice(data);
    png.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

/// A zlib stream using stored (uncompressed) deflate blocks, so no compressor
/// is needed.
fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    if data.is_empty() {
        out.extend_from_slice(&[0x01, 0x00, 0x00, 0xff, 0xff]);
    }
    let blocks = data.len().div_ceil(0xffff);
    for (index, block) in data.chunks(0xffff).enumerate() {
        out.push(u8::from(index + 1 == blocks));
        let len = block.len() as u16;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(block);
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

fn adler32(data: &[u8]) -> u32 {
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &byte in data {
        a = (a + u32::from(byte)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}
