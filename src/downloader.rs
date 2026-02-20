use std::{fs::File, io::Write, path::Path};

use anyhow::{Ok, anyhow};
use tokio::task::JoinSet;
use zip::{ZipWriter, write::SimpleFileOptions};

use crate::parser::ParseResult;

// TODO: get the extension by `Content-Type`
pub async fn save_images(result: ParseResult) -> anyhow::Result<()> {
    let ParseResult::Images { title, urls } = result else {
        return Err(anyhow!("wrong param: expecting `ParseResult::Images`"));
    };

    let width = {
        let mut n = urls.len();
        if n == 0 {
            1
        } else {
            let mut count = 0;
            while n > 0 {
                count += 1;
                n /= 10;
            }
            count
        }
    };
    let mut tasks = JoinSet::new();
    for (i, url) in urls.into_iter().enumerate() {
        tasks.spawn(async move {
            Ok((
                format!(
                    "{i:0>width$}{}",
                    Path::new(&url)
                        .extension()
                        .map(|ext| format!(".{}", ext.display()))
                        .unwrap_or_default()
                ),
                reqwest::get(&url).await?.bytes().await?,
            ))
        });
    }

    let mut results = tasks
        .join_all()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    results.sort();

    let mut writer = ZipWriter::new(File::create(format!("{title}.zip"))?);
    let options = SimpleFileOptions::default();
    for (name, res) in results {
        writer.start_file(format!("{title}/{name}"), options)?;
        writer.write_all(&res)?;
    }
    writer.finish()?;

    Ok(())
}
