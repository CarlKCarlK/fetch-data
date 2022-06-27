#![warn(missing_docs)]

//! Need more docs cmk

/// Used to construct global FetchHash object. cmk see examples
pub use ctor::ctor;
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

/// Used to fetch files from a remote location. cmk see examples
pub struct FetchHash {
    mutex: Mutex<Result<Internals, FetchHashError>>,
}

impl FetchHash {
    /// Create a new FetchHash object. cmk see examples
    pub fn new<SR, SU, SE, SQ, SO, SA>(
        registry_contents: SR,
        url_root: SU,
        env_key: SE,
        qualifier: SQ,
        organization: SO,
        application: SA,
    ) -> FetchHash
    where
        SR: AsRef<str>,
        SU: AsRef<str>,
        SE: AsRef<str>,
        SQ: AsRef<str>,
        SO: AsRef<str>,
        SA: AsRef<str>,
    {
        FetchHash {
            mutex: Mutex::new(Internals::new(
                registry_contents.as_ref(),
                url_root.as_ref(),
                env_key.as_ref(),
                qualifier.as_ref(),
                organization.as_ref(),
                application.as_ref(),
            )),
        }
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
            fetch(url, &hash, &local_path)?;
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

    /// Compute the contents of a registry file by downloading items and hashing them.
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
            let hash = hash_file(&local_path)?;
            s.push_str(&format!("{} {hash}\n", path.display()));
        }

        Ok(s)
    }
}

/// All possible errors returned by this library and the libraries it depends on.
// Based on `<https://nick.groenen.me/posts/rust-error-handling/#the-library-error-type>`
#[derive(Error, Debug)]
pub enum FetchHashError {
    #[allow(missing_docs)]
    #[error(transparent)]
    FetchHashError(#[from] FetchHashSpecificError),

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

/// If necessary, retrieve a file from a URL.
/// # Example
/// ```
/// use fetch_hash::{fetch, tmp_dir};
///
/// // Create a temporary local directory.
/// let tmp_dir = tmp_dir()?;
/// // Download the file and check its hash.
/// let path = tmp_dir.join("small.fam");
/// fetch(
///     "https://raw.githubusercontent.com/CarlKCarlK/fetch-hash/main/tests/data/small.fam",
///     "36e0086c0353ff336d0533330dbacb12c75e37dc3cba174313635b98dfe86ed2",
///     &path,
/// )?;
/// assert!(&path.exists());
/// // This time, because the local file exists and has the correct hash, no download is performed.
/// fetch(
///     "https://raw.githubusercontent.com/CarlKCarlK/fetch-hash/main/tests/data/small.fam",
///     "36e0086c0353ff336d0533330dbacb12c75e37dc3cba174313635b98dfe86ed2",
///     &path,
/// )?;
/// assert!(&path.exists());
/// # use fetch_hash::FetchHashError;
/// # Ok::<(), FetchHashError>(())
/// ```
pub fn fetch<U: AsRef<str>, H: AsRef<str>, P: AsRef<Path>>(
    url: U,
    hash: H,
    path: P,
) -> Result<(), FetchHashError> {
    let path = path.as_ref();
    if !path.exists() {
        download(url, &path)?;
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

/// Download a file from a URL and compute its hash.
///
/// # Example
/// ```
/// use fetch_hash::{hash_download, tmp_dir};
///
/// // Create a temporary local directory.
/// let tmp_dir = tmp_dir()?;
/// let path = tmp_dir.join("small.fam");
/// // Download a file and compute its hash.
/// let hash = hash_download(
///     "https://raw.githubusercontent.com/CarlKCarlK/fetch-hash/main/tests/data/small.fam",
///    &path,
/// )?;
/// assert!(hash.eq("36e0086c0353ff336d0533330dbacb12c75e37dc3cba174313635b98dfe86ed2"));
/// # use fetch_hash::FetchHashError;
/// # Ok::<(), FetchHashError>(())
/// ```
pub fn hash_download<U: AsRef<str>, P: AsRef<Path>>(
    url: U,
    path: P,
) -> Result<String, FetchHashError> {
    let path = path.as_ref();
    download(url, &path)?;
    hash_file(&path)
}

/// Compute the SHA256 hash of a local file.
///
/// # Example
/// ```
/// use fetch_hash::{hash_file, download, tmp_dir};
///
/// // Download a file to a temporary directory.
/// let tmp_dir = tmp_dir()?;
/// let path = tmp_dir.join("small.fam");
/// download(
///     "https://raw.githubusercontent.com/CarlKCarlK/fetch-hash/main/tests/data/small.fam",
///     &path,
/// )?;
/// // Compute the hash of the file.
/// let hash = hash_file(&path)?;
/// assert!(hash.eq("36e0086c0353ff336d0533330dbacb12c75e37dc3cba174313635b98dfe86ed2"));
/// # use fetch_hash::FetchHashError;
/// # Ok::<(), FetchHashError>(())
pub fn hash_file<P: AsRef<Path>>(path: P) -> Result<String, FetchHashError> {
    let mut sha256 = Sha256::new();
    let mut file = File::open(path)?;

    std::io::copy(&mut file, &mut sha256)?;
    let hash_bytes = sha256.finalize();

    let hex_hash = base16ct::lower::encode_string(&hash_bytes);
    Ok(hex_hash)
}

/// Download a file from a URL.
///
/// # Example
/// ```
/// use fetch_hash::{download, tmp_dir};
///
/// // Create a temporary local directory.
/// let tmp_dir = tmp_dir()?;
/// // Download a file to the temporary directory.
/// let path = tmp_dir.join("small.fam");
/// download(
///     "https://raw.githubusercontent.com/CarlKCarlK/fetch-hash/main/tests/data/small.fam",
///     &path,
/// )?;
/// assert!(path.exists());
/// # use fetch_hash::FetchHashError;
/// # Ok::<(), FetchHashError>(())
/// ```
pub fn download<S: AsRef<str>, P: AsRef<Path>>(url: S, path: P) -> Result<(), FetchHashError> {
    let path = path.as_ref();
    let req = ureq::get(url.as_ref()).call()?;
    let mut reader = req.into_reader();
    let mut file = File::create(&path)?;
    std::io::copy(&mut reader, &mut file)?;
    if !path.exists() {
        return Err(
            FetchHashSpecificError::DownloadedFileNotSeen(path.display().to_string()).into(),
        );
    }
    Ok(())
}

fn hash_registry(registry_contents: &str) -> Result<HashMap<PathBuf, String>, FetchHashError> {
    let mut hash_map = HashMap::new();
    for line in registry_contents.lines() {
        let mut parts = line.split_whitespace();

        let url = if let Some(url) = parts.next() {
            if url.is_empty() {
                return Err(FetchHashSpecificError::RegistryProblem().into());
            }
            PathBuf::from(url)
        } else {
            return Err(FetchHashSpecificError::RegistryProblem().into());
        };
        let hash = if let Some(hash) = parts.next() {
            hash.to_string()
        } else {
            return Err(FetchHashSpecificError::RegistryProblem().into());
        };
        if hash.is_empty() || parts.next().is_some() {
            return Err(FetchHashSpecificError::RegistryProblem().into());
        }

        hash_map.insert(url, hash.to_owned());
    }
    Ok(hash_map)
}

/// List all the files in a local directory.
///
/// # Example
/// ```
/// use fetch_hash::{dir_to_file_list, download, tmp_dir};
///
/// // Create a local directory and download two files to it.
/// let tmp_dir = tmp_dir()?;
/// download(
///     "https://raw.githubusercontent.com/CarlKCarlK/fetch-hash/main/tests/data/small.fam",
///     tmp_dir.join("small.fam"),
/// )?;
/// download(
///     "https://raw.githubusercontent.com/CarlKCarlK/fetch-hash/main/tests/data/small.bim",
///     tmp_dir.join("small.bim"),
/// )?;
/// // List the files in the directory.
/// let file_list = dir_to_file_list(tmp_dir)?;
/// println!("{file_list:?}"); // prints ["small.bim", "small.fam"]
/// # use fetch_hash::FetchHashError;
/// # Ok::<(), FetchHashError>(())
/// ```
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
        url_root: &str,
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

/// Return a path to a temporary local directory.
/// # Example
/// ```
/// use fetch_hash::{download, tmp_dir};
///
/// // Create a temporary local directory.
/// let tmp_dir = tmp_dir()?;
/// // Download a file to the temporary directory.
/// let path = tmp_dir.join("small.fam");
/// download(
///     "https://raw.githubusercontent.com/CarlKCarlK/fetch-hash/main/tests/data/small.fam",
///     &path,
/// )?;
/// assert!(path.exists());
/// # use fetch_hash::FetchHashError;
/// # Ok::<(), FetchHashError>(())
/// ```
pub fn tmp_dir() -> Result<PathBuf, FetchHashError> {
    let output_path = TempDir::default().as_ref().to_owned();
    fs::create_dir(&output_path)?;
    Ok(output_path)
}
