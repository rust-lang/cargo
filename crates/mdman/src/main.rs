use anyhow::{bail, format_err, Context, Error};
use mdman::{Format, ManMap};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use url::Url;

/// Command-line options.
struct Options {
    format: Format,
    output_dir: PathBuf,
    sources: Vec<PathBuf>,
    url: Option<Url>,
    man_map: ManMap,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {}", e);
        for cause in e.chain().skip(1) {
            eprintln!("\nCaused by:");
            for line in cause.to_string().lines() {
                if line.is_empty() {
                    eprintln!();
                } else {
                    eprintln!("  {}", line);
                }
            }
        }
        std::process::exit(1);
    }
}

fn run() -> Result<(), Error> {
    let opts = process_args()?;
    if !opts.output_dir.exists() {
        std::fs::create_dir_all(&opts.output_dir).with_context(|| {
            format!(
                "failed to create output directory {}",
                opts.output_dir.display()
            )
        })?;
    }
    for source in &opts.sources {
        let section = mdman::extract_section(source)?;
        let filename =
            Path::new(source.file_name().unwrap()).with_extension(opts.format.extension(section));
        let out_path = opts.output_dir.join(filename);
        if same_file::is_same_file(source, &out_path).unwrap_or(false) {
            bail!("cannot output to the same file as the source");
        }
        eprintln!("Converting {} -> {}", source.display(), out_path.display());
        let result = mdman::convert(&source, opts.format, opts.url.clone(), opts.man_map.clone())
            .with_context(|| format!("failed to translate {}", source.display()))?;

        std::fs::write(out_path, result)?;
    }
    Ok(())
}

fn process_args() -> Result<Options, Error> {
    let mut format = None;
    let mut output = None;
    let mut url = None;
    let mut man_map: ManMap = HashMap::new();
    let mut sources = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-t" => {
                format = match args.next().as_deref() {
                    Some("man") => Some(Format::Man),
                    Some("md") => Some(Format::Md),
                    Some("txt") => Some(Format::Text),
                    Some(s) => bail!("unknown output format: {}", s),
                    None => bail!("-t requires a value (man, md, txt)"),
                };
            }
            "-o" => {
                output = match args.next() {
                    Some(s) => Some(PathBuf::from(s)),
                    None => bail!("-o requires a value"),
                };
            }
            "--url" => {
                url = match args.next() {
                    Some(s) => {
                        let url = Url::parse(&s)
                            .with_context(|| format!("could not convert `{}` to a url", s))?;
                        if !url.path().ends_with('/') {
                            bail!("url `{}` should end with a /", url);
                        }
                        Some(url)
                    }
                    None => bail!("--url requires a value"),
                }
            }
            "--man" => {
                let man = args
                    .next()
                    .ok_or_else(|| format_err!("--man requires a value"))?;
                let parts = man.split_once('=').ok_or_else(|| {
                    anyhow::format_err!("--man expected value with form name:1=link")
                })?;
                let key_parts = parts.0.split_once(':').ok_or_else(|| {
                    anyhow::format_err!("--man expected value with form name:1=link")
                })?;
                let section: u8 = key_parts.1.parse().with_context(|| {
                    format!("expected unsigned integer for section, got `{}`", parts.1)
                })?;
                man_map.insert((key_parts.0.to_string(), section), parts.1.to_string());
            }
            s => {
                sources.push(PathBuf::from(s));
            }
        }
    }
    if format.is_none() {
        bail!("-t must be specified (man, md, txt)");
    }
    if output.is_none() {
        bail!("-o must be specified (output directory)");
    }
    if sources.is_empty() {
        bail!("at least one source must be specified");
    }
    let opts = Options {
        format: format.unwrap(),
        output_dir: output.unwrap(),
        sources,
        url,
        man_map,
    };
    Ok(opts)
}
