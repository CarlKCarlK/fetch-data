use fetch_hash::{
    ctor, dir_to_file_list, fetch, hash_download, tmp_path, FetchHash, FetchHashError,
};
use std::path::{Path, PathBuf};

// cmk Here we set up to parse at run time. We could/should parse at compile time. See:
// https://stackoverflow.com/questions/50553370/how-do-i-use-include-str-for-multiple-files-or-an-entire-directory

// !!!cmk do users need to bring in ctor, too?
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
fn one_off_fetch() -> Result<(), FetchHashError> {
    let temp_out = tmp_path()?;
    let output_file = temp_out.join("test_download_hash.fam");
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
    let temp_out = tmp_path()?;
    let output_file = temp_out.join("test_download_hash.fam");
    let actual_hash = hash_download(
        "https://raw.githubusercontent.com/CarlKCarlK/fetch-hash/main/tests/data/small.fam",
        &output_file,
    )?;
    assert!(actual_hash.eq("36e0086c0353ff336d0533330dbacb12c75e37dc3cba174313635b98dfe86ed2"));
    Ok(())
}
// !!!cmk test the delayed error result
