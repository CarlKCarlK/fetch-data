use fetch_hash::{BedErrorPlus, S1};

#[ctor::ctor]
static STATIC_SAMPLES: S1 = S1::new();

#[test]
fn one() -> Result<(), BedErrorPlus> {
    let path = STATIC_SAMPLES.sample_file("small.bim")?;
    assert!(path.exists());
    Ok(())
}
