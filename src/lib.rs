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
    fmt::Write as _,
    fs::{self, read_dir, File},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::Duration,
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
    /// Create a new `FetchData` object.
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

    fn lock(&self) -> std::sync::MutexGuard<'_, Result<Internals, Box<FetchDataError>>> {
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
            let _ = writeln!(s, "{} {hash}", path.display());
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
    let mut actual_hash = hash_file(path)?;
    if !actual_hash.eq(hash) {
        // A stale or corrupt local copy: replace it once before reporting a mismatch.
        download(url, path)?;
        actual_hash = hash_file(path)?;
    }
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
///
/// Transient failures (network errors, timeouts, HTTP 429 and 5xx) are retried
/// up to three times with exponential backoff. The file is written under a
/// temporary name and renamed into place only when complete, so an interrupted
/// download never leaves a partial file at `path`.
#[anyinput]
pub fn download(url: AnyString, path: AnyPath) -> Result<(), Box<FetchDataError>> {
    let mut attempt = 1;
    loop {
        match download_once(url, path) {
            Ok(()) => break,
            Err(e) if attempt < DOWNLOAD_ATTEMPTS && is_transient(&e) => {
                std::thread::sleep(RETRY_BASE_DELAY * (1 << (attempt - 1)));
                attempt += 1;
            }
            Err(e) => return Err(e),
        }
    }
    if !path.exists() {
        return Err(Box::new(
            FetchDataSpecificError::DownloadedFileNotSeen(path.display().to_string()).into(),
        ));
    }
    Ok(())
}

/// A download is tried this many times in all before its error is returned.
const DOWNLOAD_ATTEMPTS: u32 = 4;
/// Wait before the first retry; doubled before each later one.
#[cfg(not(test))]
const RETRY_BASE_DELAY: Duration = Duration::from_secs(1);
#[cfg(test)]
const RETRY_BASE_DELAY: Duration = Duration::from_millis(10);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
/// Longest wait for more data before an attempt is abandoned and retried.
const READ_TIMEOUT: Duration = Duration::from_secs(60);

fn download_once(url: &str, path: &Path) -> Result<(), Box<FetchDataError>> {
    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(CONNECT_TIMEOUT)
        .timeout_read(READ_TIMEOUT)
        .build();
    let response = agent.get(url).call().map_err(|e| Box::new(e.into()))?;

    let mut temp_name = path.file_name().unwrap_or_default().to_os_string();
    temp_name.push(format!(
        ".{}-{}.part",
        std::process::id(),
        NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let temp_path = path.with_file_name(temp_name);

    let result = (|| -> std::io::Result<()> {
        let mut file = File::create(&temp_path)?;
        std::io::copy(&mut response.into_reader(), &mut file)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp_path, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result.map_err(|e| Box::new(e.into()))
}

fn is_transient(error: &FetchDataError) -> bool {
    match error {
        FetchDataError::UreqError(e) => match e {
            ureq::Error::Status(code, _) => *code == 429 || *code >= 500,
            ureq::Error::Transport(_) => true,
        },
        // Reading the response body failed partway (reset, timeout, truncation).
        FetchDataError::IOError(e) => matches!(
            e.kind(),
            std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::ConnectionAborted
                | std::io::ErrorKind::BrokenPipe
                | std::io::ErrorKind::TimedOut
                | std::io::ErrorKind::UnexpectedEof
                | std::io::ErrorKind::Interrupted
                | std::io::ErrorKind::WouldBlock
        ),
        FetchDataError::FetchDataError(_) => false,
    }
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

/// A sample `sample_file`. Don't use this. Instead, define your own `sample_file` function
/// that knows how to fetch your data files.
#[anyinput]
pub fn sample_file(path: AnyPath) -> Result<PathBuf, Box<FetchDataError>> {
    STATIC_FETCH_DATA.fetch_file(path)
}

#[cfg(test)]
mod download_tests {
    use super::{download, fetch, hash_file, DOWNLOAD_ATTEMPTS};
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::thread::{self, JoinHandle};
    use temp_testdir::TempDir;

    enum Reply {
        Status(u16),
        Body(&'static [u8]),
        /// Promise `len` bytes but send only `body` before closing.
        Truncated(usize, &'static [u8]),
    }

    /// Serve one scripted reply per connection on 127.0.0.1, then stop.
    fn serve(replies: Vec<Reply>) -> (String, JoinHandle<usize>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/small.fam", listener.local_addr().unwrap());
        let handle = thread::spawn(move || {
            let mut served = 0;
            for reply in replies {
                let (mut stream, _) = listener.accept().unwrap();
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut line = String::new();
                while reader.read_line(&mut line).unwrap() > 2 {
                    line.clear();
                }
                let response = match reply {
                    Reply::Status(code) => {
                        format!("HTTP/1.1 {code} Simulated\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").into_bytes()
                    }
                    Reply::Body(body) => [
                        format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).as_bytes(),
                        body,
                    ]
                    .concat(),
                    Reply::Truncated(len, body) => [
                        format!("HTTP/1.1 200 OK\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n").as_bytes(),
                        body,
                    ]
                    .concat(),
                };
                stream.write_all(&response).unwrap();
                served += 1;
            }
            served
        });
        (url, handle)
    }

    fn part_files(dir: &std::path::Path) -> Vec<String> {
        std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| {
                std::path::Path::new(n)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("part"))
            })
            .collect()
    }

    #[test]
    fn retries_server_errors() {
        let (url, server) = serve(vec![
            Reply::Status(503),
            Reply::Status(502),
            Reply::Body(b"hello"),
        ]);
        let temp_dir = TempDir::default();
        let path = temp_dir.join("small.fam");
        download(&url, &path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"hello");
        assert_eq!(server.join().unwrap(), 3);
    }

    #[test]
    fn does_not_retry_not_found() {
        let (url, server) = serve(vec![Reply::Status(404)]);
        let temp_dir = TempDir::default();
        let path = temp_dir.join("small.fam");
        assert!(download(&url, &path).is_err());
        assert_eq!(server.join().unwrap(), 1);
        assert!(!path.exists());
    }

    #[test]
    fn truncated_body_is_retried_without_leaving_partial_file() {
        let (url, server) = serve(vec![
            Reply::Truncated(10, b"0123"),
            Reply::Body(b"complete"),
        ]);
        let temp_dir = TempDir::default();
        let path = temp_dir.join("small.fam");
        std::fs::write(&path, b"old").unwrap();
        download(&url, &path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"complete");
        assert_eq!(server.join().unwrap(), 2);
        assert_eq!(part_files(&temp_dir), Vec::<String>::new());
    }

    #[test]
    fn gives_up_and_keeps_previous_file() {
        let replies = (0..DOWNLOAD_ATTEMPTS).map(|_| Reply::Status(503)).collect();
        let (url, server) = serve(replies);
        let temp_dir = TempDir::default();
        let path = temp_dir.join("small.fam");
        std::fs::write(&path, b"old").unwrap();
        assert!(download(&url, &path).is_err());
        assert_eq!(server.join().unwrap(), DOWNLOAD_ATTEMPTS as usize);
        assert_eq!(std::fs::read(&path).unwrap(), b"old");
        assert_eq!(part_files(&temp_dir), Vec::<String>::new());
    }

    #[test]
    fn fetch_replaces_corrupt_local_file() {
        let (url, server) = serve(vec![Reply::Body(b"good")]);
        let temp_dir = TempDir::default();
        let good = temp_dir.join("good");
        std::fs::write(&good, b"good").unwrap();
        let hash = hash_file(&good).unwrap();
        let path = temp_dir.join("small.fam");
        std::fs::write(&path, b"partial from an old interrupted download").unwrap();
        fetch(&url, &hash, &path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"good");
        assert_eq!(server.join().unwrap(), 1);
    }
}
