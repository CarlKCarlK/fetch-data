#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(missing_docs)]
#![allow(clippy::missing_errors_doc)]
#![doc = include_str!("../README.md")]

use anyinput::anyinput;
/// Used to construct global `FetchData` instance.
///
/// This is a re-export from crate [`ctor`](https://crates.io/crates/ctor).
pub use ctor::ctor;
use directories::ProjectDirs;

use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::{self, read_dir, File},
    path::PathBuf,
    sync::Mutex,
};
use thiserror::Error;

/// Used to fetch data files from a URL, if needed. It verifies file contents via a hash.
///
/// # Thread Safety
///
/// `FetchData` works well with multithreaded testing, It is thread safe (via a Mutex).
///
pub struct FetchData {
    mutex: Mutex<Result<Internals, Box<FetchDataError>>>,
}

impl FetchData {
    /// Create a new FetchData object.
    ///
    /// # Errors
    ///
    /// To make `FetchData` work well as a static global, `new` never fails. Instead, `FetchData` stores any error
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
    /// use fetch_data::{FetchData};
    ///
    /// // Create a new FetchData instance.
    /// let fetch_data = FetchData::new(
    ///     "small.fam 36e0086c0353ff336d0533330dbacb12c75e37dc3cba174313635b98dfe86ed2
    ///      small.bim 56b6657a3766e2e52273f89d28be6135f9424ca1d204d29f3fa1c5a90eca794e",
    ///     "https://raw.githubusercontent.com/CarlKCarlK/fetch-data/main/tests/data/",
    ///     "BAR_APP_DATA_DIR",
    ///     "com",
    ///     "Foo Corp",
    ///     "Bar App",
    ///     );
    ///
    /// // If the local file exists and has the right hash, just return its path.
    /// // Otherwise, download the file, confirm its hash, and return its path.
    /// let local_path = fetch_data.fetch_file("small.bim")?;
    /// assert!(local_path.exists());
    /// # use fetch_data::FetchDataError;
    /// # Ok::<(), Box<FetchDataError>>(())
    /// ```
    #[anyinput]
    pub fn new(
        registry_contents: AnyString,
        url_root: AnyString,
        env_key: AnyString,
        qualifier: AnyString,
        organization: AnyString,
        application: AnyString,
    ) -> Self {
        Self {
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

    fn lock(&self) -> std::sync::MutexGuard<Result<Internals, Box<FetchDataError>>> {
        match self.mutex.lock() {
            Ok(lock) => lock,
            Err(err) => err.into_inner(),
        }
    }

    /// Fetch data files from a URL, but only if needed. Verify contents via a hash.
    ///
    /// # Example
    /// ```
    /// use fetch_data::{FetchData};
    ///
    /// // Create a new FetchData object.
    /// let fetch_data = FetchData::new(
    ///     "small.fam 36e0086c0353ff336d0533330dbacb12c75e37dc3cba174313635b98dfe86ed2
    ///      small.bim 56b6657a3766e2e52273f89d28be6135f9424ca1d204d29f3fa1c5a90eca794e",
    ///     "https://raw.githubusercontent.com/CarlKCarlK/fetch-data/main/tests/data/",
    ///     "BAR_APP_DATA_DIR",
    ///     "com",
    ///     "Foo Corp",
    ///     "Bar App",
    ///     );
    ///
    /// // If the local file exists and has the right hash, just return its path.
    /// // Otherwise, download the file, confirm its hash, and return its path.
    /// let local_path = fetch_data.fetch_file("small.bim")?;
    /// assert!(local_path.exists());
    /// # use fetch_data::FetchDataError;
    /// # Ok::<(), Box<FetchDataError>>(())
    /// ```
    #[anyinput]
    pub fn fetch_file(&self, path: AnyPath) -> Result<PathBuf, Box<FetchDataError>> {
        let path_list = vec![path.to_path_buf()];
        let vec = self.fetch_files(path_list)?;
        Ok(vec[0].clone())
    }

    /// Given a list of files, returns a list of their local paths. If necessary, the files will be downloaded.
    ///
    /// # Example
    /// ```
    /// use fetch_data::{FetchData};
    ///
    /// // Create a new FetchData instance.
    /// let fetch_data = FetchData::new(
    ///     "small.fam 36e0086c0353ff336d0533330dbacb12c75e37dc3cba174313635b98dfe86ed2
    ///      small.bim 56b6657a3766e2e52273f89d28be6135f9424ca1d204d29f3fa1c5a90eca794e",
    ///     "https://raw.githubusercontent.com/CarlKCarlK/fetch-data/main/tests/data/",
    ///     "BAR_APP_DATA_DIR",
    ///     "com",
    ///     "Foo Corp",
    ///     "Bar App",
    ///     );
    ///
    /// // If a local file exists and has the right hash, just return its path
    /// // in a list. Otherwise, download the file, confirm its hash, and return
    /// //  its path in the list.
    /// let local_path_list = fetch_data.fetch_files(["small.bim", "small.bim"])?;
    /// assert!(local_path_list[0].exists() && local_path_list[1].exists());
    /// # use fetch_data::FetchDataError;
    /// # Ok::<(), Box<FetchDataError>>(())
    /// ```
    #[anyinput]
    #[allow(clippy::significant_drop_tightening)]
    pub fn fetch_files(
        &self,
        path_list: AnyIter<AnyPath>,
    ) -> Result<Vec<PathBuf>, Box<FetchDataError>> {
        let lock = self.lock();
        // Convert Result to reference the error inside the Box for compatibility with internals()
        let lock_ref = lock.as_ref().map_err(|e| &**e);
        let internals = Self::internals(lock_ref)?;
        let hash_registry = &internals.hash_registry;
        let cache_dir = &internals.cache_dir;
        let url_root = &internals.url_root;

        let mut local_list: Vec<PathBuf> = Vec::new();
        for path in path_list {
            let path = path.as_ref();

            let path_as_string = path.to_str().ok_or_else(|| {
                Box::new(FetchDataSpecificError::UnknownOrBadFile("???".to_string()).into())
            })?;

            let Some(hash) = hash_registry.get(path) else {
                return Err(Box::new(
                    FetchDataSpecificError::UnknownOrBadFile(path_as_string.to_string()).into(),
                ));
            };

            let local_path = cache_dir.join(path);
            let url = format!("{url_root}{path_as_string}");
            fetch(url, hash, &local_path)?;
            local_list.push(local_path);
        }

        Ok(local_list)
    }

    fn internals<'a>(
        lock_ref: Result<&'a Internals, &FetchDataError>,
    ) -> Result<&'a Internals, Box<FetchDataError>> {
        match lock_ref {
            Ok(internals) => Ok(internals),
            Err(e) => Err(Box::new(
                FetchDataSpecificError::FetchDataNewFailed(e.to_string()).into(),
            )),
        }
    }
    /// Compute registry contents by downloading items and hashing them.
    ///
    /// # Tips
    ///
    /// * If you put the returned contents into a file, you can use Rust's [`std::include_str`](https://doc.rust-lang.org/std/macro.include_str.html)
    ///   macro to include the contents of that file in [`FetchData::new`](struct.FetchData.html#method.new).
    ///
    /// * Use utility function [`fetch_data::dir_to_file_list`](fn.dir_to_file_list.html) to create a list of files in any local directory.
    /// Note the hash is computed on download files, not any original local files.
    ///
    /// # Example
    ///
    /// ```
    /// use fetch_data::{FetchData};
    ///
    /// // Create a new FetchData object.
    /// let fetch_data = FetchData::new(
    ///     "", // ignored
    ///     "https://raw.githubusercontent.com/CarlKCarlK/fetch-data/main/tests/data/",
    ///     "BAR_APP_DATA_DIR",
    ///     "com",
    ///     "Foo Corp",
    ///     "Bar App",
    ///     );
    ///
    /// // Even if local files exist, download each file. Hash each file. Return the results as a string.
    /// let registry_contents = fetch_data.gen_registry_contents(["small.fam", "small.bim"])?;
    /// println!("{registry_contents}"); // Prints:
    ///                                  // small.fam 36e0086c0353ff336d0533330dbacb12c75e37dc3cba174313635b98dfe86ed2
    ///                                  // small.bim 56b6657a3766e2e52273f89d28be6135f9424ca1d204d29f3fa1c5a90eca794e
    /// # use fetch_data::FetchDataError;
    /// # Ok::<(), Box<FetchDataError>>(())
    /// ```
    #[anyinput]
    #[allow(clippy::significant_drop_tightening)]
    pub fn gen_registry_contents(
        &self,
        path_list: AnyIter<AnyPath>,
    ) -> Result<String, Box<FetchDataError>> {
        let lock = self.lock();
        // Convert Result to reference the error inside the Box for compatibility with internals()
        let lock_ref = lock.as_ref().map_err(|e| &**e);
        let internals = Self::internals(lock_ref)?;
        let cache_dir = &internals.cache_dir;
        let url_root = &internals.url_root;

        let mut s = String::new();
        for path in path_list {
            let path = path.as_ref();

            let Some(path_as_string) = path.to_str() else {
                return Err(Box::new(
                    FetchDataSpecificError::UnknownOrBadFile("???".to_string()).into(),
                ));
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
    #[allow(clippy::significant_drop_tightening)]
    pub fn cache_dir(&self) -> Result<PathBuf, Box<FetchDataError>> {
        let lock = self.lock();
        // Convert Result to reference the error inside the Box for compatibility with internals()
        let lock_ref = lock.as_ref().map_err(|e| &**e);
        let internals = Self::internals(lock_ref)?;
        let cache_dir = &internals.cache_dir;
        Ok(cache_dir.to_owned())
    }
}

/// All possible errors returned by this crate and the crates it depends on.
// Based on `<https://nick.groenen.me/posts/rust-error-handling/#the-library-error-type>`
#[derive(Error, Debug)]
pub enum FetchDataError {
    #[allow(missing_docs)]
    #[error(transparent)]
    FetchDataError(#[from] FetchDataSpecificError),

    #[allow(missing_docs)]
    #[error(transparent)]
    IOError(#[from] std::io::Error),

    #[allow(missing_docs)]
    #[error(transparent)]
    UreqError(#[from] ureq::Error),
}
/// All errors specific to this crate.
#[derive(Error, Debug, Clone)]
pub enum FetchDataSpecificError {
    #[allow(missing_docs)]
    #[error("Unknown or bad file '{0}'")]
    UnknownOrBadFile(String),

    #[allow(missing_docs)]
    #[error("The registry of files is invalid")]
    RegistryProblem(),

    #[allow(missing_docs)]
    #[error("FetchData new failed with error: {0}")]
    FetchDataNewFailed(String),

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
/// use fetch_data::fetch;
/// use temp_testdir::TempDir;
///
/// // Create a temporary local directory.
/// let temp_dir = TempDir::default();
/// // Download the file and check its hash.
/// let path = temp_dir.join("small.fam");
/// fetch(
///     "https://raw.githubusercontent.com/CarlKCarlK/fetch-data/main/tests/data/small.fam",
///     "36e0086c0353ff336d0533330dbacb12c75e37dc3cba174313635b98dfe86ed2",
///     &path,
/// )?;
/// assert!(&path.exists());
/// // This time, because the local file exists and has the correct hash, no download is performed.
/// fetch(
///     "https://raw.githubusercontent.com/CarlKCarlK/fetch-data/main/tests/data/small.fam",
///     "36e0086c0353ff336d0533330dbacb12c75e37dc3cba174313635b98dfe86ed2",
///     &path,
/// )?;
/// assert!(&path.exists());
/// # use fetch_data::FetchDataError;
/// # Ok::<(), Box<FetchDataError>>(())
/// ```
#[anyinput]
pub fn fetch(url: AnyString, hash: AnyString, path: AnyPath) -> Result<(), Box<FetchDataError>> {
    if !path.exists() {
        download(url, path)?;
    }
    let actual_hash = hash_file(path)?;
    if !actual_hash.eq(hash) {
        return Err(Box::new(
            FetchDataSpecificError::DownloadedFileWrongHash(
                path.display().to_string(),
                hash.to_string(),
                actual_hash,
            )
            .into(),
        ));
    }
    Ok(())
}

/// Download a file from a URL and compute its hash.
///
/// # Example
/// ```
/// use fetch_data::hash_download;
/// use temp_testdir::TempDir;
///
/// // Create a temporary local directory.
/// let temp_dir = TempDir::default();
/// let path = temp_dir.join("small.fam");
/// // Download a file and compute its hash.
/// let hash = hash_download(
///     "https://raw.githubusercontent.com/CarlKCarlK/fetch-data/main/tests/data/small.fam",
///    &path,
/// )?;
/// assert!(hash.eq("36e0086c0353ff336d0533330dbacb12c75e37dc3cba174313635b98dfe86ed2"));
/// # use fetch_data::FetchDataError;
/// # Ok::<(), Box<FetchDataError>>(())
/// ```
#[anyinput]
pub fn hash_download(url: AnyString, path: AnyPath) -> Result<String, Box<FetchDataError>> {
    download(url, path)?;
    hash_file(path)
}

/// Compute the hash (SHA256) of a local file.
///
/// # Example
/// ```
/// use fetch_data::{hash_file, download};
/// use temp_testdir::TempDir;
///
/// // Download a file to a temporary directory.
/// let temp_dir = TempDir::default();
/// let path = temp_dir.join("small.fam");
/// download(
///     "https://raw.githubusercontent.com/CarlKCarlK/fetch-data/main/tests/data/small.fam",
///     &path,
/// )?;
/// // Compute the hash of the file.
/// let hash = hash_file(&path)?;
/// assert!(hash.eq("36e0086c0353ff336d0533330dbacb12c75e37dc3cba174313635b98dfe86ed2"));
/// # use fetch_data::FetchDataError;
/// # Ok::<(), Box<FetchDataError>>(())
#[anyinput]
pub fn hash_file(path: AnyPath) -> Result<String, Box<FetchDataError>> {
    let mut sha256 = Sha256::new();
    let mut file = File::open(path).map_err(|e| Box::new(e.into()))?;

    std::io::copy(&mut file, &mut sha256).map_err(|e| Box::new(e.into()))?;
    let hash_bytes = sha256.finalize();

    let hex_hash = base16ct::lower::encode_string(&hash_bytes);
    Ok(hex_hash)
}

/// Download a file from a URL.
///
/// # Example
/// ```
/// use fetch_data::download;
/// use temp_testdir::TempDir;
///
/// // Create a temporary local directory.
/// let temp_dir = TempDir::default();
/// // Download a file to the temporary directory.
/// let path = temp_dir.join("small.fam");
/// download(
///     "https://raw.githubusercontent.com/CarlKCarlK/fetch-data/main/tests/data/small.fam",
///     &path,
/// )?;
/// assert!(path.exists());
/// # use fetch_data::FetchDataError;
/// # Ok::<(), Box<FetchDataError>>(())
/// ```
#[anyinput]
pub fn download(url: AnyString, path: AnyPath) -> Result<(), Box<FetchDataError>> {
    let req = ureq::get(url).call().map_err(|e| Box::new(e.into()))?;
    let mut reader = req.into_reader();
    let mut file = File::create(path).map_err(|e| Box::new(e.into()))?;
    std::io::copy(&mut reader, &mut file).map_err(|e| Box::new(e.into()))?;
    if !path.exists() {
        return Err(Box::new(
            FetchDataSpecificError::DownloadedFileNotSeen(path.display().to_string()).into(),
        ));
    }
    Ok(())
}

fn hash_registry(registry_contents: &str) -> Result<HashMap<PathBuf, String>, Box<FetchDataError>> {
    let mut hash_map = HashMap::new();
    for line in registry_contents.lines() {
        let mut parts = line.split_whitespace();

        let url = if let Some(url) = parts.next() {
            if url.is_empty() {
                return Err(Box::new(FetchDataSpecificError::RegistryProblem().into()));
            }
            PathBuf::from(url)
        } else {
            return Err(Box::new(FetchDataSpecificError::RegistryProblem().into()));
        };
        let hash = if let Some(hash) = parts.next() {
            hash.to_string()
        } else {
            return Err(Box::new(FetchDataSpecificError::RegistryProblem().into()));
        };
        if hash.is_empty() || parts.next().is_some() {
            return Err(Box::new(FetchDataSpecificError::RegistryProblem().into()));
        }

        hash_map.insert(url, hash.clone());
    }
    Ok(hash_map)
}

/// List all the files in a local directory.
///
/// # Example
/// ```
/// use fetch_data::{dir_to_file_list, download};
/// use temp_testdir::TempDir;
///
/// // Create a local directory and download two files to it.
/// let temp_dir = TempDir::default();
/// download(
///     "https://raw.githubusercontent.com/CarlKCarlK/fetch-data/main/tests/data/small.fam",
///     temp_dir.join("small.fam"),
/// )?;
/// download(
///     "https://raw.githubusercontent.com/CarlKCarlK/fetch-data/main/tests/data/small.bim",
///     temp_dir.join("small.bim"),
/// )?;
/// // List the files in the directory.
/// let file_list = dir_to_file_list(temp_dir)?;
/// println!("{file_list:?}"); // Prints ["small.bim", "small.fam"]
/// # use fetch_data::FetchDataError;
/// # Ok::<(), Box<FetchDataError>>(())
/// ```
#[anyinput]
pub fn dir_to_file_list(path: AnyPath) -> Result<Vec<std::ffi::OsString>, Box<FetchDataError>> {
    let file_list = read_dir(path)
        .map_err(|e| Box::new(e.into()))?
        .map(|res| res.map(|e| e.file_name()))
        .collect::<Result<Vec<_>, std::io::Error>>()
        .map_err(|e| Box::new(e.into()))?;
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
    ) -> Result<Self, Box<FetchDataError>> {
        let cache_dir = Self::cache_dir(env_key, qualifier, organization, application)?;
        let hash_registry = hash_registry(registry_contents)?;

        Ok(Self {
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
    ) -> Result<PathBuf, Box<FetchDataError>> {
        let cache_dir = if let Ok(cache_dir) = std::env::var(env_key) {
            PathBuf::from(cache_dir)
        } else if let Some(proj_dirs) = ProjectDirs::from(qualifier, organization, application) {
            proj_dirs.cache_dir().to_owned()
        } else {
            return Err(Box::new(
                FetchDataSpecificError::CannotCreateCacheDir().into(),
            ));
        };
        if !cache_dir.exists() {
            fs::create_dir_all(&cache_dir).map_err(|e| Box::new(e.into()))?;
        }
        Ok(cache_dir)
    }
}

#[ctor]
static STATIC_FETCH_DATA: FetchData = FetchData::new(
    include_str!("../registry.txt"),
    "https://raw.githubusercontent.com/CarlKCarlK/fetch-data/main/tests/data/",
    "BAR_APP_DATA_DIR",
    "com",
    "Foo Corp",
    "Bar App",
);

/// A sample sample_file. Don't use this. Instead, define your own `sample_file` function
/// that knows how to fetch your data files.
#[anyinput]
pub fn sample_file(path: AnyPath) -> Result<PathBuf, Box<FetchDataError>> {
    STATIC_FETCH_DATA.fetch_file(path)
}
