use fetch_hash::{BedErrorPlus, S1};

// Here we set up to parse at run time. We could/should parse at compile time. See:
// https://stackoverflow.com/questions/50553370/how-do-i-use-include-str-for-multiple-files-or-an-entire-directory
static SAMPLE_REGISTRY_CONTENTS: &str = include_str!("../tests/registry.txt");

#[ctor::ctor]
static STATIC_SAMPLES: S1 = S1::new(SAMPLE_REGISTRY_CONTENTS);

#[test]
fn one() -> Result<(), BedErrorPlus> {
    let path = STATIC_SAMPLES.sample_file("small.bim")?;
    assert!(path.exists());
    Ok(())
}
