use fetch_hash::{
    ctor, dir_to_file_list, download, fetch, hash_download, hash_file, tmp_dir, FetchHash,
    FetchHashError, FetchHashSpecificError,
};
use std::path::{Path, PathBuf};

// Here we set up to parse at run time. We could/should parse at compile time. See:
// https://stackoverflow.com/questions/50553370/how-do-i-use-include-str-for-multiple-files-or-an-entire-directory

#[ctor]
static STATIC_FETCH_HASH: FetchHash = FetchHash::new(
    include_str!("../tests/registry.txt"),
    "https://raw.githubusercontent.com/CarlKCarlK/fetch-hash/main/tests/data/",
    "BAR_APP_DATA_DIR",
    "com",
    "Foo Corp",
    "Bar App",
);

#[test]
fn static_data() -> Result<(), FetchHashError> {
    fn sample_file<P: AsRef<Path>>(path: P) -> Result<PathBuf, FetchHashError> {
        STATIC_FETCH_HASH.fetch_file(path)
    }

    let local_path = sample_file("small.bim")?;
    assert!(local_path.exists());
    Ok(())
}

// !!!cmk tell why new never fails

#[test]
fn just_in_time_data() -> Result<(), FetchHashError> {
    let fetch_hash = FetchHash::new(
        include_str!("../tests/registry.txt"),
        "https://raw.githubusercontent.com/CarlKCarlK/fetch-hash/main/tests/data/",
        "BAR_APP_DATA_DIR",
        "com",
        "Foo Corp",
        "Bar App",
    );

    let local_path = fetch_hash.fetch_file("empty.bed")?;
    assert!(local_path.exists());
    Ok(())
}

#[test]
fn create_registry_file() -> Result<(), FetchHashError> {
    // Create list of files in data directory

    let fetch_hash = FetchHash::new(
        "",
        "https://raw.githubusercontent.com/CarlKCarlK/fetch-hash/main/tests/data/",
        "BAR_APP_DATA_DIR",
        "com",
        "Foo Corp",
        "Bar App",
    );

    let file_list = dir_to_file_list("tests/data")?;
    let registry_contents = fetch_hash.gen_registry_contents(file_list)?;
    println!("{registry_contents}");
    Ok(())
}

#[test]
fn gen_registry_contents_example() -> Result<(), FetchHashError> {
    let fetch_hash = FetchHash::new(
        "", // ignored
        "https://raw.githubusercontent.com/CarlKCarlK/fetch-hash/main/tests/data/",
        "BAR_APP_DATA_DIR",
        "com",
        "Foo Corp",
        "Bar App",
    );

    let registry_contents = fetch_hash.gen_registry_contents(["small.fam", "small.bim"])?;
    println!("{registry_contents}"); // prints:
                                     // small.fam 36e0086c0353ff336d0533330dbacb12c75e37dc3cba174313635b98dfe86ed2
                                     // small.bim 56b6657a3766e2e52273f89d28be6135f9424ca1d204d29f3fa1c5a90eca794e
    Ok(())
}

#[test]
fn one_off_fetch() -> Result<(), FetchHashError> {
    let tmp_dir = tmp_dir()?;
    let output_file = tmp_dir.join("test_download_hash.fam");
    fetch(
        "https://raw.githubusercontent.com/CarlKCarlK/fetch-hash/main/tests/data/small.fam",
        "36e0086c0353ff336d0533330dbacb12c75e37dc3cba174313635b98dfe86ed2",
        &output_file,
    )?;
    assert!(output_file.exists());

    Ok(())
}

#[test]
fn one_off_hash_download() -> Result<(), FetchHashError> {
    let tmp_dir = tmp_dir()?;
    let path = tmp_dir.join("small.fam");
    let hash = hash_download(
        "https://raw.githubusercontent.com/CarlKCarlK/fetch-hash/main/tests/data/small.fam",
        &path,
    )?;
    assert!(hash.eq("36e0086c0353ff336d0533330dbacb12c75e37dc3cba174313635b98dfe86ed2"));
    Ok(())
}

#[test]
fn one_off_just_download() -> Result<(), FetchHashError> {
    let tmp_dir = tmp_dir()?;
    let path = tmp_dir.join("small.fam");
    download(
        "https://raw.githubusercontent.com/CarlKCarlK/fetch-hash/main/tests/data/small.fam",
        &path,
    )?;
    assert!(path.exists());
    Ok(())
}

#[test]
fn one_off_just_hash_file() -> Result<(), FetchHashError> {
    let tmp_dir = tmp_dir()?;
    let path = tmp_dir.join("small.fam");
    download(
        "https://raw.githubusercontent.com/CarlKCarlK/fetch-hash/main/tests/data/small.fam",
        &path,
    )?;
    let hash = hash_file(&path)?;
    assert!(hash.eq("36e0086c0353ff336d0533330dbacb12c75e37dc3cba174313635b98dfe86ed2"));
    Ok(())
}

#[test]
fn one_off_just_dir_to_file_list() -> Result<(), FetchHashError> {
    let tmp_dir = tmp_dir()?;
    download(
        "https://raw.githubusercontent.com/CarlKCarlK/fetch-hash/main/tests/data/small.fam",
        tmp_dir.join("small.fam"),
    )?;
    download(
        "https://raw.githubusercontent.com/CarlKCarlK/fetch-hash/main/tests/data/small.bim",
        tmp_dir.join("small.bim"),
    )?;
    let file_list = dir_to_file_list(tmp_dir)?;
    println!("{file_list:?}"); // prints ["small.bim", "small.fam"]
    Ok(())
}

#[test]
fn bad_fetch_hash() -> Result<(), FetchHashError> {
    // Create list of files in data directory

    let fetch_hash = FetchHash::new(
        "OneColumn",
        "",
        "BAR_APP_DATA_DIR",
        "com",
        "Foo Corp",
        "Bar App",
    );

    let result = fetch_hash.fetch_file("small.bim");

    match result {
        Err(FetchHashError::FetchHashError(FetchHashSpecificError::FetchHashNewFailed(_))) => (),
        _ => panic!("test failure"),
    };

    Ok(())
}

#[test]
fn fetch_hash_new_example() -> Result<(), FetchHashError> {
    let fetch_hash = FetchHash::new(
        "small.fam 36e0086c0353ff336d0533330dbacb12c75e37dc3cba174313635b98dfe86ed2
                           small.bim 56b6657a3766e2e52273f89d28be6135f9424ca1d204d29f3fa1c5a90eca794e",
        "https://raw.githubusercontent.com/CarlKCarlK/fetch-hash/main/tests/data/",
        "BAR_APP_DATA_DIR",
        "com",
        "Foo Corp",
        "Bar App",
    );

    let local_path = fetch_hash.fetch_file("small.bim")?;
    assert!(local_path.exists());
    Ok(())
}
