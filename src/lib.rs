#![warn(missing_docs)]
#![doc = include_str!("../README.md")]

/// Used to construct global FetchHash instance.
///
/// This is a re-export from crate [`ctor`](https://crates.io/crates/ctor).
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

/// Used to fetch data files from a URL, if needed. It verifies file contents via a hash.
///
/// # Thread Safety
///
/// `FetchHash` works well with multithreaded testing, It is thread safe (via a Mutex).
///
pub struct FetchHash {
    mutex: Mutex<Result<Internals, FetchHashError>>,
}

impl FetchHash {
    /// Create a new FetchHash object.
    ///
    /// # Errors
    ///
    /// To make `FetchHash` work well as a static global, `new` never fails. Instead, `FetchHash` stores any error
    /// and returns it when the first call to `fetch_file`, etc., is made.
    ///
    /// # Arguments
    ///  *all inputs are string-like*
    ///
    /// * `registry_contents` - Whitespace delimited list of files and hashes.
    ///           Use Rust's [`std::include_str`](https://doc.rust-lang.org/std/macro.include_str.html)
    ///           macro to include the contents of a file.
    /// * `url_root` - Base URL for remote files.
    /// * `env_key` - Environment variable that may contain the path to the data directory.
    ///           If not set, the data directory will be create via
    ///           [`ProjectDirs`](https://docs.rs/directories/latest/directories/struct.ProjectDirs.html#method.from_path)
    ///           and the next three arguments.
    /// * `qualifier` - The reverse domain name notation of the application, excluding the organization or application name itself.
    /// * `organization` - The name of the organization that develops this application.
    /// * `application` - The name of the application itself.
    ///
    /// # Example
    /// ```
    /// use fetch_hash::{FetchHash};
    ///
    /// // Create a new FetchHash instance.
    /// let fetch_hash = FetchHash::new(
    ///     "small.fam 36e0086c0353ff336d0533330dbacb12c75e37dc3cba174313635b98dfe86ed2
    ///      small.bim 56b6657a3766e2e52273f89d28be6135f9424ca1d204d29f3fa1c5a90eca794e",
    ///     "https://raw.githubusercontent.com/CarlKCarlK/fetch-hash/main/tests/data/",
    ///     "BAR_APP_DATA_DIR",
    ///     "com",
    ///     "Foo Corp",
    ///     "Bar App",
    ///     );
    ///
    /// // If the local file exists and has the right hash, just return its path.
    /// // Otherwise, download the file, confirm its hash, and return its path.
    /// let local_path = fetch_hash.fetch_file("small.bim")?;
    /// assert!(local_path.exists());
    /// # use fetch_hash::FetchHashError;
    /// # Ok::<(), FetchHashError>(())
    /// ```
    pub fn new<S0, S1, S3, S4, S5, S6>(
        registry_contents: S0,
        url_root: S1,
        env_key: S3,
        qualifier: S4,
        organization: S5,
        application: S6,
    ) -> FetchHash
    where
        // any string-like input
        S0: AsRef<str>,
        S1: AsRef<str>,
        S3: AsRef<str>,
        S4: AsRef<str>,
        S5: AsRef<str>,
        S6: AsRef<str>,
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

    /// Fetch data files from a URL, but only if needed. Verify contents via a hash.
    ///
    /// # Example
    /// ```
    /// use fetch_hash::{FetchHash};
    ///
    /// // Create a new FetchHash object.
    /// let fetch_hash = FetchHash::new(
    ///     "small.fam 36e0086c0353ff336d0533330dbacb12c75e37dc3cba174313635b98dfe86ed2
    ///      small.bim 56b6657a3766e2e52273f89d28be6135f9424ca1d204d29f3fa1c5a90eca794e",
    ///     "https://raw.githubusercontent.com/CarlKCarlK/fetch-hash/main/tests/data/",
    ///     "BAR_APP_DATA_DIR",
    ///     "com",
    ///     "Foo Corp",
    ///     "Bar App",
    ///     );
    ///
    /// // If the local file exists and has the right hash, just return its path.
    /// // Otherwise, download the file, confirm its hash, and return its path.
    /// let local_path = fetch_hash.fetch_file("small.bim")?;
    /// assert!(local_path.exists());
    /// # use fetch_hash::FetchHashError;
    /// # Ok::<(), FetchHashError>(())
    /// ```
    pub fn fetch_file<P: AsRef<Path>>(&self, path: P) -> Result<PathBuf, FetchHashError> {
        let path_list = vec![path.as_ref().to_path_buf()];
        let vec = self.fetch_files(path_list)?;
        Ok(vec[0].clone())
    }

    /// Given a list of files, returns a list of their local paths. If necessary, the files will be downloaded.
    ///
    /// # Example
    /// ```
    /// use fetch_hash::{FetchHash};
    ///
    /// // Create a new FetchHash instance.
    /// let fetch_hash = FetchHash::new(
    ///     "small.fam 36e0086c0353ff336d0533330dbacb12c75e37dc3cba174313635b98dfe86ed2
    ///      small.bim 56b6657a3766e2e52273f89d28be6135f9424ca1d204d29f3fa1c5a90eca794e",
    ///     "https://raw.githubusercontent.com/CarlKCarlK/fetch-hash/main/tests/data/",
    ///     "BAR_APP_DATA_DIR",
    ///     "com",
    ///     "Foo Corp",
    ///     "Bar App",
    ///     );
    ///
    /// // If a local file exists and has the right hash, just return its path
    /// // in a list. Otherwise, download the file, confirm its hash, and return
    /// //  its path in the list.
    /// let local_path_list = fetch_hash.fetch_files(["small.bim", "small.bim"])?;
    /// assert!(local_path_list[0].exists() && local_path_list[1].exists());
    /// # use fetch_hash::FetchHashError;
    /// # Ok::<(), FetchHashError>(())
    /// ```
    pub fn fetch_files<I, P>(&self, path_list: I) -> Result<Vec<PathBuf>, FetchHashError>
    where
        // Any list-like iterable of path-like items
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

    /// Compute registry contents by downloading items and hashing them.
    ///
    /// # Tips
    ///
    /// * If you put the returned contents into a file, you can use Rust's [`std::include_str`](https://doc.rust-lang.org/std/macro.include_str.html)
    ///   macro to include the contents of that file in [`FetchHash::new`](struct.FetchHash.html#method.new).
    ///
    /// * Use utility function [`fetch_hash::dir_to_file_list`](fn.dir_to_file_list.html) to create a list of files in any local directory.
    /// Note the hash is computed on download files, not any original local files.
    ///
    /// # Example
    ///
    /// ```
    /// use fetch_hash::{FetchHash};
    ///
    /// // Create a new FetchHash object.
    /// let fetch_hash = FetchHash::new(
    ///     "", // ignored
    ///     "https://raw.githubusercontent.com/CarlKCarlK/fetch-hash/main/tests/data/",
    ///     "BAR_APP_DATA_DIR",
    ///     "com",
    ///     "Foo Corp",
    ///     "Bar App",
    ///     );
    ///
    /// // Even if local files exist, download each file. Hash each file. Return the results as a string.
    /// let registry_contents = fetch_hash.gen_registry_contents(["small.fam", "small.bim"])?;
    /// println!("{registry_contents}"); // Prints:
    ///                                  // small.fam 36e0086c0353ff336d0533330dbacb12c75e37dc3cba174313635b98dfe86ed2
    ///                                  // small.bim 56b6657a3766e2e52273f89d28be6135f9424ca1d204d29f3fa1c5a90eca794e
    /// # use fetch_hash::FetchHashError;
    /// # Ok::<(), FetchHashError>(())
    /// ```
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

    /// Return the path to the local cache directory.
    pub fn cache_dir(&self) -> Result<PathBuf, FetchHashError> {
        let lock = self.lock();
        let internals = FetchHash::internals(lock.as_ref())?;
        let cache_dir = &internals.cache_dir;
        Ok(cache_dir.to_owned())
    }
}

/// All possible errors returned by this crate and the crates it depends on.
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
/// All errors specific to this crate.
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

/// If necessary, retrieve a file from a URL, checking its hash.
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

/// Compute the hash (SHA256) of a local file.
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
/// println!("{file_list:?}"); // Prints ["small.bim", "small.fam"]
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

#[ctor]
static STATIC_FETCH_HASH: FetchHash = FetchHash::new(
    include_str!("../registry.txt"),
    "https://raw.githubusercontent.com/CarlKCarlK/fetch-hash/main/tests/data/",
    "BAR_APP_DATA_DIR",
    "com",
    "Foo Corp",
    "Bar App",
);

/// A sample sample_file. Don't use this. Instead, define your own `sample_file` function
/// that knows how to fetch your data files.
pub fn sample_file<P: AsRef<Path>>(path: P) -> Result<PathBuf, FetchHashError> {
    STATIC_FETCH_HASH.fetch_file(path)
}
