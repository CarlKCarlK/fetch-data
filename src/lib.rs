// !!!cmk #![warn(missing_docs)]

// !!! cmk rename Samples and BedError BED_READER bedreader bed-reader
// !!! cmk rename this project to be fetch-hash

use directories::ProjectDirs;
use std::{
    collections::HashMap,
    fs::{self, File},
    path::{Path, PathBuf},
    sync::Mutex,
};
use thiserror::Error;

use sha2::{Digest, Sha256};

// !!! cmk why to structs?
struct Samples {
    cache_dir: PathBuf,
    hash_registry: HashMap<PathBuf, String>,
    url_root: String,
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
    #[error("Unknown or bad sample file '{0}'")]
    UnknownOrBadSampleFile(String),

    #[allow(missing_docs)]
    #[error("The registry of sample files is invalid")]
    SampleRegistryProblem(),

    #[allow(missing_docs)]
    #[error("Samples construction failed with error: {0}")]
    SamplesConstructionFailed(String),

    #[allow(missing_docs)]
    #[error("Downloaded sample file not seen: {0}")]
    DownloadedSampleFileNotSeen(String),

    #[allow(missing_docs)]
    #[error("Downloaded sample file has wrong hash: {0},expected: {1}, actual: {2}")]
    DownloadedSampleFileWrongHash(String, String, String),

    #[allow(missing_docs)]
    #[error("Cannot create cache directory")]
    CannotCreateCacheDir(),
}

pub struct FetchHash {
    mutex: Mutex<Result<Samples, FetchHashError>>,
}

impl FetchHash {
    pub fn new(
        sample_registry_contents: &str,
        url_root: &str,
        key: &str, // !!! cmk call this environment_key?
        qualifier: &str,
        organization: &str,
        application: &str,
    ) -> FetchHash {
        let cache_dir_result = FetchHash::cache_dir(key, qualifier, organization, application);
        FetchHash {
            mutex: Mutex::new(FetchHash::new_samples(
                sample_registry_contents,
                url_root,
                cache_dir_result,
            )),
        }
    }

    fn new_samples(
        sample_registry_contents: &str,
        url_root: &str, // !!! cmk String?
        cache_dir_result: Result<PathBuf, FetchHashError>,
    ) -> Result<Samples, FetchHashError> {
        let cache_dir = cache_dir_result?;
        let hash_registry = hash_registry(sample_registry_contents)?;

        Ok(Samples {
            cache_dir,
            hash_registry,
            url_root: url_root.to_string(),
        })
    }

    fn cache_dir(
        key: &str,
        qualifier: &str,
        organization: &str,
        application: &str,
    ) -> Result<PathBuf, FetchHashError> {
        // !!!cmk two keys?
        // Return BED_READER_DATA_DIR is present
        let cache_dir = if let Ok(cache_dir) = std::env::var(key) {
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

    /// Returns the local path to a sample file. If necessary, the file will be downloaded.
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
        let lock = match self.mutex.lock() {
            Ok(lock) => lock,
            Err(err) => err.into_inner(),
        };
        let samples = match lock.as_ref() {
            Ok(samples) => samples,
            Err(e) => {
                return Err(
                    FetchHashSpecificError::SamplesConstructionFailed(e.to_string()).into(),
                );
            }
        };
        let hash_registry = &samples.hash_registry;
        let cache_dir = &samples.cache_dir;
        let url_root = &samples.url_root;

        let mut local_list: Vec<PathBuf> = Vec::new();
        for path in path_list {
            let path = path.as_ref();

            let path_as_string = if let Some(path_as_string) = path.to_str() {
                path_as_string
            } else {
                return Err(
                    FetchHashSpecificError::UnknownOrBadSampleFile("???".to_string()).into(),
                );
            };

            let hash = if let Some(hash) = hash_registry.get(path) {
                hash
            } else {
                return Err(FetchHashSpecificError::UnknownOrBadSampleFile(
                    path_as_string.to_string(),
                )
                .into());
            };

            let local_path = cache_dir.join(path);
            let url = format!("{url_root}{path_as_string}");
            download_hash(url, &hash, &local_path)?;
            local_list.push(local_path);
        }

        Ok(local_list)
    }
}

// https://stackoverflow.com/questions/58006033/how-to-run-setup-code-before-any-tests-run-in-rust
fn download_hash<U: AsRef<str>, H: AsRef<str>, P: AsRef<Path>>(
    url: U,
    hash: H,
    path: P,
) -> Result<(), FetchHashError> {
    let path = path.as_ref();
    if !path.exists() {
        download(url, &path)?;
        if !path.exists() {
            return Err(FetchHashSpecificError::DownloadedSampleFileNotSeen(
                path.display().to_string(),
            )
            .into());
        }
    }
    let actual_hash = hash_file(&path)?;
    if !actual_hash.eq(hash.as_ref()) {
        return Err(FetchHashSpecificError::DownloadedSampleFileWrongHash(
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

fn hash_registry(
    sample_registry_contents: &str,
) -> Result<HashMap<PathBuf, String>, FetchHashError> {
    let mut hash_map = HashMap::new();
    for line in sample_registry_contents.lines() {
        let mut parts = line.split_whitespace();

        let url = if let Some(url) = parts.next() {
            PathBuf::from(url)
        } else {
            return Err(FetchHashSpecificError::SampleRegistryProblem().into());
        };
        let hash = if let Some(hash) = parts.next() {
            hash.to_string()
        } else {
            return Err(FetchHashSpecificError::SampleRegistryProblem().into());
        };
        if parts.next().is_some() {
            return Err(FetchHashSpecificError::SampleRegistryProblem().into());
        }

        hash_map.insert(url, hash.to_owned());
    }
    Ok(hash_map)
}
