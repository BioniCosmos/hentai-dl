use std::io::Write;

use anyhow::Result;
use tar::{Builder, EntryType, Header};
use zip::{DateTime, ZipWriter, write::SimpleFileOptions};

pub(crate) fn media_type_to_ext(media_type: &str) -> Option<&str> {
    const M: &[(&str, &str)] = &[
        ("image/avif", "avif"),
        ("image/bmp", "bmp"),
        ("image/heic", "heic"),
        ("image/jpeg", "jpeg"),
        ("image/png", "png"),
        ("image/webp", "webp"),
    ];
    M.iter()
        .find(|(t, _)| media_type.contains(t))
        .map(|(_, ext)| *ext)
}

pub(crate) fn tar(writer: impl Write, files: &[(String, Box<[u8]>)], dir: &str) -> Result<()> {
    struct Path {
        name: String,
        prefix: Option<String>,
        path: Option<String>,
    }
    fn truncate_dir(name: &str) -> Path {
        if name.len() <= 100 {
            Path {
                name: format!("{name}/"),
                prefix: None,
                path: None,
            }
        } else {
            Path {
                name: format!("{}/", &name[..99]),
                prefix: None,
                path: Some(format!("{name}/")),
            }
        }
    }
    fn truncate_file(name: &str, dir: &Path) -> Path {
        let pax = dir.path.is_some() || name.len() > 100;
        let split = name.len() + dir.name.len() > 100;
        let path = if pax {
            Some(format!(
                "{}{name}",
                if let Some(path) = &dir.path {
                    path
                } else {
                    &dir.name
                }
            ))
        } else {
            None
        };
        if split {
            let start = if name.len() > 100 {
                name.len() - 100
            } else {
                0
            };
            Path {
                name: name[start..name.len()].to_owned(),
                prefix: Some(dir.name.trim_end_matches('/').to_owned()),
                path,
            }
        } else {
            Path {
                name: format!("{}{name}", dir.name),
                prefix: None,
                path,
            }
        }
    }

    fn pax(builder: &mut Builder<impl Write>, path: &str) -> Result<()> {
        let base = 7 + path.len();
        let mut len = base + 1;
        loop {
            let n = base + len.ilog10() as usize + 1;
            if n == len {
                break;
            }
            len = n;
        }
        let data = format!("{len} path={path}\n");

        let mut header = Header::new_ustar();
        let mut name = format!("PAX/{path}");
        name.truncate(100);
        header.as_old_mut().name[..name.len()].copy_from_slice(name.as_bytes());
        header.set_size(data.len() as u64);
        header.set_entry_type(EntryType::XHeader);
        header.set_cksum();
        builder.append(&header, data.as_bytes())?;
        Ok(())
    }

    let mut builder = Builder::new(writer);

    let dir = truncate_dir(dir);
    let mut header = Header::new_ustar();
    header.as_old_mut().name[..dir.name.len()].copy_from_slice(dir.name.as_bytes());
    if let Some(path) = &dir.path {
        pax(&mut builder, path)?;
    }
    header.set_size(0);
    header.set_entry_type(EntryType::Directory);
    header.set_uid(0);
    header.set_gid(0);
    header.set_mode(0o755);
    header.set_cksum();
    builder.append(&header, &[][..])?;

    for (name, data) in files {
        let file = truncate_file(name, &dir);
        let mut header = Header::new_ustar();
        header.as_old_mut().name[..file.name.len()].copy_from_slice(file.name.as_bytes());
        if let Some(prefix) = file.prefix {
            header.as_ustar_mut().unwrap().prefix[..prefix.len()]
                .copy_from_slice(prefix.as_bytes());
        }
        if let Some(path) = file.path {
            pax(&mut builder, &path)?;
        }
        header.set_size(data.len() as u64);
        header.set_entry_type(EntryType::Regular);
        header.set_uid(0);
        header.set_gid(0);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append(&header, &data[..])?;
    }

    builder.finish()?;
    Ok(())
}

pub(crate) fn zip(writer: impl Write, files: &[(String, Box<[u8]>)], dir: &str) -> Result<()> {
    let mut writer = ZipWriter::new_stream(writer);
    let options = SimpleFileOptions::default();
    let options = options.last_modified_time(DateTime::DEFAULT);
    writer.add_directory(format!("{dir}/"), options)?;
    for (name, data) in files {
        writer.start_file(format!("{dir}/{name}"), options)?;
        writer.write_all(&data)?;
    }
    writer.finish()?;
    Ok(())
}
