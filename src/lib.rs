// !!!cmk #![warn(missing_docs)]

use directories::ProjectDirs;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::{self, read_dir, File},
    path::{Path, PathBuf},
    sync::Mutex,
};
use temp_testdir::TempDir;
use thiserror::Error;

pub struct FetchHash {
    mutex: Mutex<Result<Internals, FetchHashError>>,
}

impl FetchHash {
    pub fn new(
        registry_contents: &str,
        url_root: &str, // !!! cmk String?
        env_key: &str,
        qualifier: &str,
        organization: &str,
        application: &str,
    ) -> FetchHash {
        FetchHash {
            mutex: Mutex::new(Internals::new(
                registry_contents,
                url_root,
                env_key,
                qualifier,
                organization,
                application,
            )),
        }
    }

    pub fn gen_registry_contents<I, P>(&self, path_list: I) -> Result<String, FetchHashError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        let lock = self.lock();
        let internals = FetchHash::internals(lock.as_ref())?;
        let cache_dir = &internals.cache_dir;
        let url_root = &internals.url_root;

        let mut s = String::new();
        for path in path_list {
            let path = path.as_ref();

            let path_as_string = if let Some(path_as_string) = path.to_str() {
                path_as_string
            } else {
                return Err(FetchHashSpecificError::UnknownOrBadFile("???".to_string()).into());
            };

            let local_path = cache_dir.join(path);
            let url = format!("{url_root}{path_as_string}");
            download(url, &local_path)?;
            if !local_path.exists() {
                return Err(FetchHashSpecificError::DownloadedFileNotSeen(
                    local_path.display().to_string(),
                )
                .into());
            }
            let hash = hash_file(&local_path)?;
            s.push_str(&format!("{} {hash}\n", path.display()));
        }

        Ok(s)
    }

    fn lock(&self) -> std::sync::MutexGuard<Result<Internals, FetchHashError>> {
        let lock = match self.mutex.lock() {
            Ok(lock) => lock,
            Err(err) => err.into_inner(),
        };
        lock
    }

    /// Returns the local path to a file. If necessary, the file will be downloaded.
    ///
    /// A SHA256 hash is used to verify that the file is correct.
    /// The file will be in a directory determined by environment variable `BED_READER_DATA_DIR`.
    /// If that environment variable is not set, a cache folder, appropriate to the OS, will be used.
    pub fn fetch_file<P: AsRef<Path>>(&self, path: P) -> Result<PathBuf, FetchHashError> {
        let path_list = vec![path.as_ref().to_path_buf()];
        let vec = self.fetch_files(path_list)?;
        Ok(vec[0].clone())
    }
    /// Returns the local paths to a list of files. If necessary, the files will be downloaded.
    ///
    /// SHA256 hashes are used to verify that the files are correct.
    /// The files will be in a directory determined by environment variable `BED_READER_DATA_DIR`.
    /// If that environment variable is not set, a cache folder, appropriate to the OS, will be used.
    pub fn fetch_files<I, P>(&self, path_list: I) -> Result<Vec<PathBuf>, FetchHashError>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        let lock = self.lock();
        let internals = FetchHash::internals(lock.as_ref())?;
        let hash_registry = &internals.hash_registry;
        let cache_dir = &internals.cache_dir;
        let url_root = &internals.url_root;

        let mut local_list: Vec<PathBuf> = Vec::new();
        for path in path_list {
            let path = path.as_ref();

            let path_as_string = if let Some(path_as_string) = path.to_str() {
                path_as_string
            } else {
                return Err(FetchHashSpecificError::UnknownOrBadFile("???".to_string()).into());
            };

            let hash = if let Some(hash) = hash_registry.get(path) {
                hash
            } else {
                return Err(
                    FetchHashSpecificError::UnknownOrBadFile(path_as_string.to_string()).into(),
                );
            };

            let local_path = cache_dir.join(path);
            let url = format!("{url_root}{path_as_string}");
            download_hash(url, &hash, &local_path)?;
            local_list.push(local_path);
        }

        Ok(local_list)
    }

    fn internals<'a>(
        lock_ref: Result<&'a Internals, &FetchHashError>,
    ) -> Result<&'a Internals, FetchHashError> {
        match lock_ref {
            Ok(internals) => Ok(internals),
            Err(e) => Err(FetchHashSpecificError::FetchHashNewFailed(e.to_string()).into()),
        }
    }
}

/// All possible errors returned by this library and the libraries it depends on.
// Based on `<https://nick.groenen.me/posts/rust-error-handling/#the-library-error-type>`
#[derive(Error, Debug)]
pub enum FetchHashError {
    #[allow(missing_docs)]
    #[error(transparent)]
    BedError(#[from] FetchHashSpecificError),

    #[allow(missing_docs)]
    #[error(transparent)]
    IOError(#[from] std::io::Error),

    #[allow(missing_docs)]
    #[error(transparent)]
    UreqError(#[from] ureq::Error),
}
/// All errors specific to this library.
#[derive(Error, Debug, Clone)]
pub enum FetchHashSpecificError {
    #[allow(missing_docs)]
    #[error("Unknown or bad file '{0}'")]
    UnknownOrBadFile(String),

    #[allow(missing_docs)]
    #[error("The registry of files is invalid")]
    RegistryProblem(),

    #[allow(missing_docs)]
    #[error("FetchHash new failed with error: {0}")]
    FetchHashNewFailed(String),

    #[allow(missing_docs)]
    #[error("Downloaded file not seen: {0}")]
    DownloadedFileNotSeen(String),

    #[allow(missing_docs)]
    #[error("Downloaded file has wrong hash: {0},expected: {1}, actual: {2}")]
    DownloadedFileWrongHash(String, String, String),

    #[allow(missing_docs)]
    #[error("Cannot create cache directory")]
    CannotCreateCacheDir(),
}

// https://stackoverflow.com/questions/58006033/how-to-run-setup-code-before-any-tests-run-in-rust
pub fn download_hash<U: AsRef<str>, H: AsRef<str>, P: AsRef<Path>>(
    url: U,
    hash: H,
    path: P,
) -> Result<(), FetchHashError> {
    let path = path.as_ref();
    if !path.exists() {
        download(url, &path)?;
        if !path.exists() {
            return Err(
                FetchHashSpecificError::DownloadedFileNotSeen(path.display().to_string()).into(),
            );
        }
    }
    let actual_hash = hash_file(&path)?;
    if !actual_hash.eq(hash.as_ref()) {
        return Err(FetchHashSpecificError::DownloadedFileWrongHash(
            path.display().to_string(),
            hash.as_ref().to_string(),
            actual_hash,
        )
        .into());
    }
    Ok(())
}

fn hash_file<P: AsRef<Path>>(path: P) -> Result<String, FetchHashError> {
    let mut sha256 = Sha256::new();
    let mut file = File::open(path)?;

    std::io::copy(&mut file, &mut sha256)?;
    let hash_bytes = sha256.finalize();

    let hex_hash = base16ct::lower::encode_string(&hash_bytes);
    Ok(hex_hash)
}

fn download<S: AsRef<str>, P: AsRef<Path>>(url: S, file_path: P) -> Result<(), FetchHashError> {
    let req = ureq::get(url.as_ref()).call()?;
    let mut reader = req.into_reader();
    let mut file = File::create(&file_path)?;
    std::io::copy(&mut reader, &mut file)?;
    Ok(())
}

fn hash_registry(registry_contents: &str) -> Result<HashMap<PathBuf, String>, FetchHashError> {
    let mut hash_map = HashMap::new();
    for line in registry_contents.lines() {
        let mut parts = line.split_whitespace();

        let url = if let Some(url) = parts.next() {
            PathBuf::from(url)
        } else {
            return Err(FetchHashSpecificError::RegistryProblem().into());
        };
        let hash = if let Some(hash) = parts.next() {
            hash.to_string()
        } else {
            return Err(FetchHashSpecificError::RegistryProblem().into());
        };
        if parts.next().is_some() {
            return Err(FetchHashSpecificError::RegistryProblem().into());
        }

        hash_map.insert(url, hash.to_owned());
    }
    Ok(hash_map)
}

pub fn dir_to_file_list<P: AsRef<Path>>(
    path: P,
) -> Result<Vec<std::ffi::OsString>, FetchHashError> {
    let file_list = read_dir(path)?
        .map(|res| res.map(|e| e.file_name()))
        .collect::<Result<Vec<_>, std::io::Error>>()?;
    Ok(file_list)
}
struct Internals {
    cache_dir: PathBuf,
    hash_registry: HashMap<PathBuf, String>,
    url_root: String,
}

impl Internals {
    fn new(
        registry_contents: &str,
        url_root: &str, // !!! cmk String?
        env_key: &str,
        qualifier: &str,
        organization: &str,
        application: &str,
    ) -> Result<Internals, FetchHashError> {
        let cache_dir = Internals::cache_dir(env_key, qualifier, organization, application)?;
        let hash_registry = hash_registry(registry_contents)?;

        Ok(Internals {
            cache_dir,
            hash_registry,
            url_root: url_root.to_string(),
        })
    }

    fn cache_dir(
        env_key: &str,
        qualifier: &str,
        organization: &str,
        application: &str,
    ) -> Result<PathBuf, FetchHashError> {
        let cache_dir = if let Ok(cache_dir) = std::env::var(env_key) {
            PathBuf::from(cache_dir)
        } else if let Some(proj_dirs) = ProjectDirs::from(qualifier, organization, application) {
            proj_dirs.cache_dir().to_owned()
        } else {
            return Err(FetchHashSpecificError::CannotCreateCacheDir().into());
        };
        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir)?;
        }
        Ok(cache_dir)
    }
}

/// Return a path to a temporary directory. //!!!cmk update example
///
/// # Example
/// ```
// / use ndarray as nd;
// / use bed_reader::{tmp_path, WriteOptions};
// / let output_folder = tmp_path()?;
// / let output_file = output_folder.join("small.bed");
// / let val = nd::array![
// /     [1.0, 0.0, f64::NAN, 0.0],
// /     [2.0, 0.0, f64::NAN, 2.0],
// /     [0.0, 1.0, 2.0, 0.0]
// / ];
// / WriteOptions::builder(output_file).write(&val)?;
// / # use bed_reader::BedErrorPlus;
// / # Ok::<(), BedErrorPlus>(())
/// ```
pub fn tmp_path() -> Result<PathBuf, FetchHashError> {
    let output_path = TempDir::default().as_ref().to_owned();
    fs::create_dir(&output_path)?;
    Ok(output_path)
}
