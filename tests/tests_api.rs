use std::path::{Path, PathBuf};

use fetch_data::{
    ctor, dir_to_file_list, download, fetch, hash_download, hash_file, FetchData, FetchDataError,
    FetchDataSpecificError,
};
use temp_testdir::TempDir;

#[test]
fn static_data() -> Result<(), FetchDataError> {
    let local_path = sample_file("small.bim")?;
    assert!(local_path.exists());
    Ok(())
}

#[test]
fn just_in_time_data() -> Result<(), FetchDataError> {
    let fetch_data = FetchData::new(
        include_str!("../registry.txt"),
        "https://raw.githubusercontent.com/CarlKCarlK/fetch-data/main/tests/data/",
        "BAR_APP_DATA_DIR",
        "com",
        "Foo Corp",
        "Bar App",
    );

    let local_path = fetch_data.fetch_file("empty.bed")?;
    assert!(local_path.exists());
    Ok(())
}

#[test]
fn create_registry_file() -> Result<(), FetchDataError> {
    // Create list of files in data directory

    let fetch_data = FetchData::new(
        "",
        "https://raw.githubusercontent.com/CarlKCarlK/fetch-data/main/tests/data/",
        "BAR_APP_DATA_DIR",
        "com",
        "Foo Corp",
        "Bar App",
    );

    let file_list = dir_to_file_list("tests/data")?;
    let registry_contents = fetch_data.gen_registry_contents(file_list)?;
    println!("{registry_contents}");
    Ok(())
}

#[test]
fn gen_registry_contents_example() -> Result<(), FetchDataError> {
    let fetch_data = FetchData::new(
        "", // ignored
        "https://raw.githubusercontent.com/CarlKCarlK/fetch-data/main/tests/data/",
        "BAR_APP_DATA_DIR",
        "com",
        "Foo Corp",
        "Bar App",
    );

    let registry_contents = fetch_data.gen_registry_contents(["small.fam", "small.bim"])?;
    println!("{registry_contents}"); // Prints:
                                     // small.fam 36e0086c0353ff336d0533330dbacb12c75e37dc3cba174313635b98dfe86ed2
                                     // small.bim 56b6657a3766e2e52273f89d28be6135f9424ca1d204d29f3fa1c5a90eca794e
    Ok(())
}

#[test]
fn cache_dir() -> Result<(), FetchDataError> {
    let cache_dir = STATIC_TEST_API.cache_dir()?;
    assert!(cache_dir.exists());
    println!("{cache_dir:?}",);
    Ok(())
}

#[test]
fn one_off_fetch() -> Result<(), FetchDataError> {
    let temp_dir = TempDir::default();
    let output_file = temp_dir.join("small.fam");
    fetch(
        "https://raw.githubusercontent.com/CarlKCarlK/fetch-data/main/tests/data/small.fam",
        "36e0086c0353ff336d0533330dbacb12c75e37dc3cba174313635b98dfe86ed2",
        &output_file,
    )?;
    assert!(output_file.exists());

    Ok(())
}

#[test]
fn one_off_hash_download() -> Result<(), FetchDataError> {
    let temp_dir = TempDir::default();
    let path = temp_dir.join("small.fam");
    let hash = hash_download(
        "https://raw.githubusercontent.com/CarlKCarlK/fetch-data/main/tests/data/small.fam",
        &path,
    )?;
    assert!(hash.eq("36e0086c0353ff336d0533330dbacb12c75e37dc3cba174313635b98dfe86ed2"));
    Ok(())
}

#[test]
fn one_off_just_download() -> Result<(), FetchDataError> {
    let temp_dir = TempDir::default();
    let path = temp_dir.join("small.fam");
    download(
        "https://raw.githubusercontent.com/CarlKCarlK/fetch-data/main/tests/data/small.fam",
        &path,
    )?;
    assert!(path.exists());
    Ok(())
}

#[test]
fn one_off_just_hash_file() -> Result<(), FetchDataError> {
    let temp_dir = TempDir::default();
    let path = temp_dir.join("small.fam");
    download(
        "https://raw.githubusercontent.com/CarlKCarlK/fetch-data/main/tests/data/small.fam",
        &path,
    )?;
    let hash = hash_file(&path)?;
    assert!(hash.eq("36e0086c0353ff336d0533330dbacb12c75e37dc3cba174313635b98dfe86ed2"));
    Ok(())
}

#[test]
fn one_off_just_dir_to_file_list() -> Result<(), FetchDataError> {
    let temp_dir = TempDir::default();
    download(
        "https://raw.githubusercontent.com/CarlKCarlK/fetch-data/main/tests/data/small.fam",
        temp_dir.join("small.fam"),
    )?;
    download(
        "https://raw.githubusercontent.com/CarlKCarlK/fetch-data/main/tests/data/small.bim",
        temp_dir.join("small.bim"),
    )?;
    let file_list = dir_to_file_list(temp_dir)?;
    println!("{file_list:?}"); // Prints ["small.bim", "small.fam"]
    Ok(())
}

#[test]
fn bad_fetch_data() -> Result<(), FetchDataError> {
    // Create list of files in data directory

    let fetch_data = FetchData::new(
        "OneColumn",
        "",
        "BAR_APP_DATA_DIR",
        "com",
        "Foo Corp",
        "Bar App",
    );

    let result = fetch_data.fetch_file("small.bim");

    match result {
        Err(FetchDataError::FetchDataError(FetchDataSpecificError::FetchDataNewFailed(_))) => (),
        _ => panic!("test failure"),
    };

    Ok(())
}

#[test]
fn fetch_data_new_example() -> Result<(), FetchDataError> {
    let fetch_data = FetchData::new(
        "small.fam 36e0086c0353ff336d0533330dbacb12c75e37dc3cba174313635b98dfe86ed2
                           small.bim 56b6657a3766e2e52273f89d28be6135f9424ca1d204d29f3fa1c5a90eca794e",
        "https://raw.githubusercontent.com/CarlKCarlK/fetch-data/main/tests/data/",
        "BAR_APP_DATA_DIR",
        "com",
        "Foo Corp",
        "Bar App",
    );

    let local_path = fetch_data.fetch_file("small.bim")?;
    assert!(local_path.exists());
    Ok(())
}

#[test]
fn readme_example1() -> Result<(), FetchDataError> {
    use fetch_data::sample_file;

    let path = sample_file("small.fam")?;
    println!("{}", std::fs::metadata(path)?.len()); // Prints 85
    Ok(())
}

#[ctor]
static STATIC_TEST_API: FetchData = FetchData::new(
    include_str!("../registry.txt"),
    "https://raw.githubusercontent.com/CarlKCarlK/fetch-data/main/tests/data/",
    "BAR_APP_DATA_DIR",
    "com",
    "Foo Corp",
    "Bar App",
);

/// A sample sample_file. Don't use this. Instead, define your own that knows
/// how to fetch your data files.
pub fn sample_file<P: AsRef<Path>>(path: P) -> Result<PathBuf, FetchDataError> {
    STATIC_TEST_API.fetch_file(path)
}
