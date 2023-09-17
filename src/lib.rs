use chrono::Local;
use cxx::CxxString;
use std::{
    fs::{self, File},
    io::{BufWriter, Read, Write},
    iter::Iterator,
    path::Path,
};
use time::{OffsetDateTime, UtcOffset};
use walkdir::WalkDir;
use zip::{result::ZipError, write::FileOptions, DateTime};

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

fn backup_save(save_path: &str, zip_dir_path: &str) -> zip::result::ZipResult<()> {
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
    let zip_name = format!("{}-{}.zip", save_name, formatted_date_time);
    let zip_path = zip_dir_path.join(zip_name);
    let zip_file = BufWriter::new(File::create(zip_path)?);

    let save_path_walkdir = WalkDir::new(save_path).into_iter().filter_map(|e| e.ok());

    let local_utcoffset = UtcOffset::current_local_offset().unwrap();

    let mut zip = zip::ZipWriter::new(zip_file);

    let mut buffer = Vec::new();
    for entry in save_path_walkdir {
        let path = entry.path();
        let path_datetime: OffsetDateTime = path.metadata()?.modified()?.into();
        let path_inside_zip = path.strip_prefix(save_base_path).unwrap();

        let zip_options = FileOptions::default()
            .compression_method(zip::CompressionMethod::Zstd)
            .unix_permissions(0o755)
            .last_modified_time(
                DateTime::try_from(path_datetime.to_offset(local_utcoffset)).unwrap(),
            );

        if path.is_file() {
            zip.start_file(path_as_string(path_inside_zip), zip_options)?;
            let mut file = File::open(path)?;
            file.read_to_end(&mut buffer)?;
            zip.write_all(&buffer)?;
            buffer.clear();
        } else if !path.as_os_str().is_empty() {
            zip.add_directory(path_as_string(path_inside_zip), zip_options)?;
        }
    }
    zip.finish()?;
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
