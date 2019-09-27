use clap::{load_yaml, App, ArgMatches};
use std::path::{Path, PathBuf};
use tempfile::{tempdir, TempDir};

pub struct ConstatOptions {
    pub repo_path: PathBuf,
    pub top: usize,
    pub out_path: PathBuf,
    pub resolution: (u32, u32),
    _temp_file_handle: Option<TempDir>,
}

impl ConstatOptions {
    pub fn new() -> Self {
        let option_spec = load_yaml!("cli.yml");
        let options = App::from_yaml(option_spec).get_matches();

        let (repo_path, handle) = get_repo_path(&options);
        let out_path = get_out_path(&options, repo_path.as_ref());

        Self {
            repo_path,
            top: get_num_tops(&options),
            out_path,
            resolution: get_resolution(&options),
            _temp_file_handle: handle,
        }
    }
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

    eprintln!("Cloning remote repo into temp dir {:?}", path);

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
