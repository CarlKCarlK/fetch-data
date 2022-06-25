use std::path::{Path, PathBuf};

use fetch_hash::{dir_to_file_list, FetchHash, FetchHashError};

// Here we set up to parse at run time. We could/should parse at compile time. See:
// https://stackoverflow.com/questions/50553370/how-do-i-use-include-str-for-multiple-files-or-an-entire-directory
static FETCH_HASH_REGISTRY_CONTENTS: &str = include_str!("../tests/registry.txt");

#[ctor::ctor]
static STATIC_FETCH_HASH: FetchHash = FetchHash::new(
    FETCH_HASH_REGISTRY_CONTENTS,
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

// !!! cmk show how to generate registry.txt
