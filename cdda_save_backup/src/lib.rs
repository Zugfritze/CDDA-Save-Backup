use chrono::Local;
use cxx::CxxString;
use parking_lot::Mutex;
use rayon::iter::{IntoParallelIterator, ParallelBridge, ParallelIterator};
use std::{
    fs::{self, File},
    io::{BufReader, BufWriter, Read, Write},
    iter::Iterator,
    path::{Path, PathBuf},
};
use time::{OffsetDateTime, UtcOffset};
use walkdir::WalkDir;
use zip::{
    result::{ZipError, ZipResult},
    write::FileOptions,
    DateTime, ZipArchive, ZipWriter,
};
use zstd::stream::read::{Decoder, Encoder};

#[cxx::bridge]
mod ffi {
    extern "Rust" {
        fn cxx_backup_save(save_path: &CxxString, zip_path: &CxxString) -> bool;
    }
}

pub fn cxx_backup_save(save_path: &CxxString, zip_dir_path: &CxxString) -> bool {
    let save_path = match save_path.to_str() {
        Ok(path) => path,
        Err(_) => return false,
    };
    let zip_path = match zip_dir_path.to_str() {
        Ok(path) => path,
        Err(_) => return false,
    };
    backup_save(save_path, zip_path).is_ok()
}

fn backup_save(save_path: &str, zip_dir_path: &str) -> ZipResult<()> {
    let save_path = Path::new(save_path);
    if !save_path.is_dir() {
        return Err(ZipError::FileNotFound);
    }

    let save_base_path = save_path.parent().unwrap();

    let save_name = save_path
        .file_name()
        .ok_or(ZipError::FileNotFound)?
        .to_string_lossy()
        .to_string();

    let zip_dir_path = Path::new(zip_dir_path).join(&save_name);
    if !zip_dir_path.is_dir() {
        fs::create_dir_all(&zip_dir_path)?;
    }

    let formatted_date_time = Local::now().format("[%Y-%m-%d-%H%M%S]").to_string();
    let zip_name = format!("{}-{}.savebackup", save_name, formatted_date_time);
    let zip_path = zip_dir_path.join(zip_name);
    let zip_file = BufWriter::new(File::create(zip_path)?);

    let local_utcoffset = UtcOffset::current_local_offset().unwrap();

    let zip_mutex = Mutex::new(ZipWriter::new(zip_file));

    let save_path_walkdir = WalkDir::new(save_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .par_bridge();

    save_path_walkdir
        .into_par_iter()
        .try_for_each(|entry| -> ZipResult<()> {
            let path = entry.path();
            let path_datetime: OffsetDateTime = path.metadata()?.modified()?.into();
            let path_inside_zip = path.strip_prefix(save_base_path).unwrap();

            let zip_options = FileOptions::default()
                .compression_method(zip::CompressionMethod::Stored)
                .unix_permissions(0o755)
                .last_modified_time(
                    DateTime::try_from(path_datetime.to_offset(local_utcoffset)).unwrap(),
                );

            if path.is_file() {
                let file = File::open(path)?;
                let mut buffer = Vec::new();
                let mut zstd_encoder = Encoder::new(file, 1)?;
                zstd_encoder.read_to_end(&mut buffer)?;
                let mut zip = zip_mutex.lock();
                zip.start_file(path_as_string(path_inside_zip), zip_options)?;
                zip.write_all(&buffer)?;
            } else if !path.as_os_str().is_empty() {
                zip_mutex
                    .lock()
                    .add_directory(path_as_string(path_inside_zip), zip_options)?;
            }

            Ok(())
        })?;

    zip_mutex.lock().finish()?;
    Ok(())
}

pub fn read_backup_save(backup_save_path: &str, output_directory: Option<&str>) -> ZipResult<()> {
    let zip = BufReader::new(File::open(backup_save_path)?);
    let mut zip = ZipArchive::new(zip)?;

    for entry in 0..zip.len() {
        let zip_entry = zip.by_index(entry)?;
        let zip_entry_path = match output_directory {
            Some(directory) => Path::new(directory).join(zip_entry.enclosed_name().unwrap()),
            None => Path::new(backup_save_path).parent().unwrap().join(zip_entry.enclosed_name().unwrap()),
        };
        if zip_entry.is_file() {
            let dir = zip_entry_path.parent().unwrap();
            if !dir.exists() {
                fs::create_dir_all(dir)?;
            }
            let mut file = File::create(zip_entry_path)?;
            let mut buffer = Vec::new();
            let mut zstd_decoder = Decoder::new(zip_entry)?;
            zstd_decoder.read_to_end(&mut buffer)?;
            file.write_all(&buffer)?;
        } else if zip_entry.is_dir() {
            fs::create_dir_all(zip_entry_path)?;
        }
    }
    Ok(())
}

fn path_as_string(path: &Path) -> String {
    path.components()
        .filter_map(|component| {
            if let std::path::Component::Normal(os_str) = component {
                Some(os_str.to_string_lossy().to_string())
            } else {
                None
            }
        })
        .collect::<Vec<String>>()
        .join("/")
}
