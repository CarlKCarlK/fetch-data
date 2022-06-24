use fetch_hash::{FetchHash, FetchHashError};

// Here we set up to parse at run time. We could/should parse at compile time. See:
// https://stackoverflow.com/questions/50553370/how-do-i-use-include-str-for-multiple-files-or-an-entire-directory
static FETCH_HASH_REGISTRY_CONTENTS: &str = include_str!("../tests/registry.txt");

#[ctor::ctor]
static STATIC_FETCH_HASH: FetchHash = FetchHash::new(
    FETCH_HASH_REGISTRY_CONTENTS,
    "https://raw.githubusercontent.com/fastlmm/bed-reader/rustybed/bed_reader/tests/data/",
    "BED_READER_DATA_DIR",
    "github.io",
    "fastlmm",
    "bed-reader",
);

#[test]
fn one() -> Result<(), FetchHashError> {
    let path = STATIC_FETCH_HASH.fetch_file("small.bim")?;
    assert!(path.exists());
    Ok(())
}
