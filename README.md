fetch-data
==========

[<img alt="github" src="https://img.shields.io/badge/github-fetch--data-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/CarlKCarlK/fetch-data)
[<img alt="crates.io" src="https://img.shields.io/crates/v/fetch-data.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/fetch-data)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-fetch--data-66c2a5?style=for-the-badge&labelColor=555555&logoColor=white&logo=data:image/svg+xml;base64,PHN2ZyByb2xlPSJpbWciIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgdmlld0JveD0iMCAwIDUxMiA1MTIiPjxwYXRoIGZpbGw9IiNmNWY1ZjUiIGQ9Ik00ODguNiAyNTAuMkwzOTIgMjE0VjEwNS41YzAtMTUtOS4zLTI4LjQtMjMuNC0zMy43bC0xMDAtMzcuNWMtOC4xLTMuMS0xNy4xLTMuMS0yNS4zIDBsLTEwMCAzNy41Yy0xNC4xIDUuMy0yMy40IDE4LjctMjMuNCAzMy43VjIxNGwtOTYuNiAzNi4yQzkuMyAyNTUuNSAwIDI2OC45IDAgMjgzLjlWMzk0YzAgMTMuNiA3LjcgMjYuMSAxOS45IDMyLjJsMTAwIDUwYzEwLjEgNS4xIDIyLjEgNS4xIDMyLjIgMGwxMDMuOS01MiAxMDMuOSA1MmMxMC4xIDUuMSAyMi4xIDUuMSAzMi4yIDBsMTAwLTUwYzEyLjItNi4xIDE5LjktMTguNiAxOS45LTMyLjJWMjgzLjljMC0xNS05LjMtMjguNC0yMy40LTMzLjd6TTM1OCAyMTQuOGwtODUgMzEuOXYtNjguMmw4NS0zN3Y3My4zek0xNTQgMTA0LjFsMTAyLTM4LjIgMTAyIDM4LjJ2LjZsLTEwMiA0MS40LTEwMi00MS40di0uNnptODQgMjkxLjFsLTg1IDQyLjV2LTc5LjFsODUtMzguOHY3NS40em0wLTExMmwtMTAyIDQxLjQtMTAyLTQxLjR2LS42bDEwMi0zOC4yIDEwMiAzOC4ydi42em0yNDAgMTEybC04NSA0Mi41di03OS4xbDg1LTM4Ljh2NzUuNHptMC0xMTJsLTEwMiA0MS40LTEwMi00MS40di0uNmwxMDItMzguMiAxMDIgMzguMnYuNnoiPjwvcGF0aD48L3N2Zz4K" height="20">](https://docs.rs/fetch-data)
[<img alt="build status" src="https://img.shields.io/github/workflow/status/CarlKCarlK/fetch-data/CI/master?style=for-the-badge" height="20">](https://github.com/CarlKCarlk/fetch-data)

Fetch data files from a URL, but only if needed. Verify contents via SHA256.

`Fetch-Data` checks a local data directory and then downloads needed files. It always verifies the local files and downloaded files via a hash.

 `Fetch-Data` makes it easy to download large and small sample files. For example, here we download a genomics file from GitHub (if it has not already been downloaded). We then print the size of the now local file.

```rust
use fetch_data::sample_file;

let path = sample_file("small.fam")?;
println!("{}", std::fs::metadata(path)?.len()); // Prints 85

# use fetch_data::FetchDataError; // '#' needed for doctest
# Ok::<(), FetchDataError>(())
```
Features
--------

* Thread-safe -- allowing it to be used with Rust's multithreaded testing framework.
* Inspired by Python's popular [Pooch](https://pypi.org/project/pooch/) and our PySnpTools [filecache module](https://fastlmm.github.io/PySnpTools/#module-pysnptools.util.filecache).
* Avoids run-times such as Tokio (by using [`ureq`](https://crates.io/crates/ureq) to download files via blocking I/O).

<a name="suggested-usage"></a>Suggested Usage
-----

You can set up [`FetchData`](https://docs.rs/fetch-data/latest/fetch_data/struct.FetchData.html) many ways. Here are the steps -- followed by sample code -- for one set up.

* Create a `registry.txt` file containing a whitespace-delimited list of files
  and their hashes. (This is the same format as [Pooch](https://pypi.org/project/pooch/). See section [Registry Creation](#registry-creation) for tips on creating this file.)

* As shown below, create a global static
 [`FetchData`](https://docs.rs/fetch-data/latest/fetch_data/struct.FetchData.html#method.new)
  instance that reads your `registry.txt` file. Give it:
  -  the URL root from which to download the files
  -  an environment variable telling the local data directory
     in which to store the files
   - a `qualifier`, `organization`, and `application` -- Used to
     create a local data 
     directory when the environment variable is not set. See crate [ProjectsDir](https://docs.rs/directories/latest/directories/struct.ProjectDirs.html#method.from_path) for details.

 * As shown below, define a public `sample_file` function that takes a file name and returns a `Result`
   containing the path to the downloaded file.

```rust
use fetch_data::{ctor, FetchData, FetchDataError};
use std::path::{Path, PathBuf};

#[ctor]
static STATIC_FETCH_DATA: FetchData = FetchData::new(
    include_str!("../registry.txt"),
    "https://raw.githubusercontent.com/CarlKCarlK/fetch-data/main/tests/data/",
    "BAR_APP_DATA_DIR", // env_key
    "com",              // qualifier
    "Foo Corp",         // organization
    "Bar App",          // application
);

/// Download a data file.
pub fn sample_file<P: AsRef<Path>>(path: P) -> Result<PathBuf, FetchDataError> {
    STATIC_FETCH_DATA.fetch_file(path)
}

```

You can now use your `sample_file` function to download your files as needed.

<a name="registry-creation"></a>Registry Creation
------------------------

You can create your `registry.txt` file many ways. Here are the steps -- followed by sample code -- for one way to create it.

* Upload your data files to the Internet.
   - For example, `Fetch-Data`
  puts its sample data files
  in `tests/data`, so they upload to [this GitHub folder](https://github.com/CarlKCarlK/fetch-data/tree/main/tests/data). In GitHub, by looking at the [raw view of a data file](https://github.com/CarlKCarlK/fetch-data/blob/main/tests/data/small.fam), we see the root URL for these files. In `cargo.toml`, we keep these data files out of our crate via `exclude = ["tests/data/*"]`
* As shown below, write code that
  - Creates a [`FetchData`](https://docs.rs/fetch-data/latest/fetch_data/struct.FetchData.html#method.new) instance without registry contents.
  - Lists the files in your data directory.
  - Calls the [`gen_registry_contents`](https://docs.rs/fetch-data/latest/fetch_data/struct.FetchData.html#method.gen_registry_contents) method on your list of files. This method will download
    the files, compute their hashes, and create a string of file names and hashes.
* Print this string, then manually paste it into a file called `registry.txt`.

```rust
use fetch_data::{FetchData, dir_to_file_list};

let fetch_data = FetchData::new(
    "", // registry_contents ignored
    "https://raw.githubusercontent.com/CarlKCarlK/fetch-data/main/tests/data/",
    "BAR_APP_DATA_DIR", // env_key
    "com",              // qualifier
    "Foo Corp",         // organization
    "Bar App",          // application
);
let file_list = dir_to_file_list("tests/data")?;
let registry_contents = fetch_data.gen_registry_contents(file_list)?;
println!("{registry_contents}");

# use fetch_data::FetchDataError; // '#' needed for doctest
# Ok::<(), FetchDataError>(())
```

Notes
-----

* Feature requests and contributions are welcome.
* Don't use our sample `sample_file`. Define your own `sample_file` that
  knows where to find *your* data files.
* The [`FetchData`](https://docs.rs/fetch-data/latest/fetch_data/struct.FetchData.html) instance need not be global and static. See [`FetchData::new`](https://docs.rs/fetch-data/latest/fetch_data/struct.FetchData.html#method.new) for an example of a non-global instance.
* Additional [`methods on the FetchData`](https://docs.rs/fetch-data/latest/fetch_data/struct.FetchData.html#implementations) instance can fetch multiples files
and can give the path to the local data directory.
* You need not use a `registry.txt` file 
and [`FetchData`](https://docs.rs/fetch-data/latest/fetch_data/struct.FetchData.html) instance. You can instead use the stand-alone function [`fetch`](https://docs.rs/fetch-data/latest/fetch_data/fn.fetch.html) to retrieve a single file with known URL, hash, and local path.
* Additional [stand-alone functions](https://docs.rs/fetch-data/latest/fetch_data/#functions) can download files and hash files.
* `Fetch-Data` always does binary downloads to maintain consistant line endings across OSs.
* The [Bed-Reader](https://github.com/fastlmm/bed-reader/tree/fetch-hash) genomics crate
uses `Fetch-Data`.
* To make `FetchData` work well as a static global,
[`FetchData::new`](https://docs.rs/fetch-data/latest/fetch_data/struct.FetchData.html#method) never fails. Instead,
`FetchData` stores any error
and returns it when the first call to `fetch_file`, etc., is made.

* Debugging this crate under Windows can cause a "Oops! The debug adapter has terminated abnormally" exception. This is some kind of [LLVM, Windows, NVIDIA(?) problem](https://github.com/vadimcn/vscode-lldb/issues/410) via ureq.
* This crate follows [Nine Rules for Elegant Rust Library APIs](https://towardsdatascience.com/nine-rules-for-elegant-rust-library-apis-9b986a465247) from *Towards Data Science*.


Project Links
-----

* [**Installation**](https://crates.io/crates/fetch-data)
* [**Documentation**](https://docs.rs/fetch-data/)
* [**Source code**](https://github.com/CarlKCarlK/fetch-data)
* [**Discussion**](https://github.com/CarlKCarlK/fetch-data/discussions/)
* [**Bug Reports and Feature Requests**](https://github.com/CarlKCarlK/fetch-data/issues)
