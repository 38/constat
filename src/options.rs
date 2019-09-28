use chrono::{Date, NaiveDate, TimeZone, Utc};
use clap::{load_yaml, value_t_or_exit, values_t_or_exit, App, ArgMatches};
use glob::Pattern;
use std::path::{Path, PathBuf};
use tempfile::{tempdir, TempDir};

use crate::stat::VersionSpec;

pub struct ConstatOptions {
    pub repo_path: PathBuf,
    pub top: usize,
    pub out_path: PathBuf,
    pub resolution: (u32, u32),
    pub patterns: Vec<Pattern>,
    pub top_only: bool,
    pub open: bool,
    pub since: Option<VersionSpec>,
    pub exclude_older: bool,
    pub quiet: bool,
    _temp_file_handle: Option<TempDir>,
}

impl ConstatOptions {
    pub fn new() -> Self {
        let option_spec = load_yaml!("cli.yml");
        let options = App::from_yaml(option_spec).get_matches();

        let (repo_path, handle) = get_repo_path(&options);
        let out_path = get_out_path(&options, repo_path.as_ref());

        let patterns = parse_patterns(&options);

        Self {
            repo_path,
            top: get_num_tops(&options),
            out_path,
            resolution: get_resolution(&options),
            patterns,
            top_only: options.is_present("top-only"),
            open: options.is_present("open"),
            since: if options.is_present("since-date") {
                Some(VersionSpec::FirstAfter(parse_date(&options, "since-date")))
            } else {
                None
            },
            exclude_older: options.is_present("exclude-older"),
            quiet: options.is_present("quiet"),
            _temp_file_handle: handle,
        }
    }
}

fn parse_date(parsed: &ArgMatches, name: &str) -> Date<Utc> {
    let nd = value_t_or_exit!(parsed.value_of(name), NaiveDate);
    Utc.from_utc_date(&nd)
}

fn parse_patterns(parsed: &ArgMatches) -> Vec<Pattern> {
    if !parsed.is_present("file-patterns") {
        return vec!["**/*".parse().unwrap()];
    }
    values_t_or_exit!(parsed.values_of("file-patterns"), Pattern)
}

fn get_repo_path(parsed: &ArgMatches) -> (PathBuf, Option<TempDir>) {
    if let Ok(path) = std::fs::canonicalize(parsed.value_of("repository").unwrap_or(".")) {
        return (path, None);
    }

    let url = parsed.value_of("repository").unwrap();

    let name = AsRef::<Path>::as_ref(url.split("/").last().unwrap())
        .file_stem()
        .unwrap();
    let temp = tempdir().unwrap();

    let mut path = temp.path().to_path_buf();

    path.push(name);

    eprintln!("Cloning remote repo into temp dir {:?} ...", path);

    git2::Repository::clone(url, &path).unwrap();

    (path, Some(temp))
}

fn get_num_tops(parsed: &ArgMatches) -> usize {
    parsed.value_of("top").unwrap_or("5").parse().unwrap()
}

fn get_out_path(parsed: &ArgMatches, repo_path: &Path) -> PathBuf {
    parsed
        .value_of("output")
        .map(|x| x.to_string())
        .unwrap_or_else(|| {
            format!(
                "{}.constat.png",
                repo_path
                    .file_name()
                    .unwrap_or_else(|| "unknown-repo".as_ref())
                    .to_string_lossy()
                    .to_owned()
            )
        })
        .into()
}

fn get_resolution(parsed: &ArgMatches) -> (u32, u32) {
    let mut parser = parsed
        .value_of("resolution")
        .unwrap_or("1024x768")
        .split(|x| x == 'x')
        .take(2)
        .map(|s| s.parse().unwrap());
    (parser.next().unwrap(), parser.next().unwrap())
}
