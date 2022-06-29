fetch-hash
==========

[<img alt="github" src="https://img.shields.io/badge/github-fetch--hash-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/CarlKCarlK/fetch-hash)
[<img alt="crates.io" src="https://img.shields.io/crates/v/fetch-hash.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/fetch-hash)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-fetch--hash-66c2a5?style=for-the-badge&labelColor=555555&logoColor=white&logo=data:image/svg+xml;base64,PHN2ZyByb2xlPSJpbWciIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgdmlld0JveD0iMCAwIDUxMiA1MTIiPjxwYXRoIGZpbGw9IiNmNWY1ZjUiIGQ9Ik00ODguNiAyNTAuMkwzOTIgMjE0VjEwNS41YzAtMTUtOS4zLTI4LjQtMjMuNC0zMy43bC0xMDAtMzcuNWMtOC4xLTMuMS0xNy4xLTMuMS0yNS4zIDBsLTEwMCAzNy41Yy0xNC4xIDUuMy0yMy40IDE4LjctMjMuNCAzMy43VjIxNGwtOTYuNiAzNi4yQzkuMyAyNTUuNSAwIDI2OC45IDAgMjgzLjlWMzk0YzAgMTMuNiA3LjcgMjYuMSAxOS45IDMyLjJsMTAwIDUwYzEwLjEgNS4xIDIyLjEgNS4xIDMyLjIgMGwxMDMuOS01MiAxMDMuOSA1MmMxMC4xIDUuMSAyMi4xIDUuMSAzMi4yIDBsMTAwLTUwYzEyLjItNi4xIDE5LjktMTguNiAxOS45LTMyLjJWMjgzLjljMC0xNS05LjMtMjguNC0yMy40LTMzLjd6TTM1OCAyMTQuOGwtODUgMzEuOXYtNjguMmw4NS0zN3Y3My4zek0xNTQgMTA0LjFsMTAyLTM4LjIgMTAyIDM4LjJ2LjZsLTEwMiA0MS40LTEwMi00MS40di0uNnptODQgMjkxLjFsLTg1IDQyLjV2LTc5LjFsODUtMzguOHY3NS40em0wLTExMmwtMTAyIDQxLjQtMTAyLTQxLjR2LS42bDEwMi0zOC4yIDEwMiAzOC4ydi42em0yNDAgMTEybC04NSA0Mi41di03OS4xbDg1LTM4Ljh2NzUuNHptMC0xMTJsLTEwMiA0MS40LTEwMi00MS40di0uNmwxMDItMzguMiAxMDIgMzguMnYuNnoiPjwvcGF0aD48L3N2Zz4K" height="20">](https://docs.rs/fetch-hash)
[<img alt="build status" src="https://img.shields.io/github/workflow/status/CarlKCarlK/fetch-hash/CI/master?style=for-the-badge" height="20">](https://github.com/CarlKCarlk/fetch-hash)

Fetch data files from a URL, but only if needed. Verify contents via SHA256.

`Fetch-Hash` downloads files only if they are not already in a local data directory. It always verifies the local files and downloaded files via a hash.

 `Fetch-Hash` makes it easy to download large and small samples files. For example, here we download a genomics file from GitHub (if it has not already been downloaded). We then print the size of the now local file.

```rust
use fetch_hash::sample_file;

let path = sample_file("small.fam")?;
println!("{}", std::fs::metadata(path)?.len()); // Prints 85

# use fetch_hash::FetchHashError; // '#' needed for doctest
# Ok::<(), FetchHashError>(())
```
Features
--------

* Thread-safe -- allowing it to be used with Rust's multithreaded testing framework.
* Inspired by Python's popular [Pooch](https://pypi.org/project/pooch/) and our PySnpTools [filecache module](https://fastlmm.github.io/PySnpTools/#module-pysnptools.util.filecache).
* Avoids run-times such a Tokio by using [`ureq`](https://crates.io/crates/ureq) to download files via blocking I/O.

<a name="suggested-usage"></a>Suggested Usage
-----

You can set up [`FetchHash`](https://docs.rs/fetch-hash/latest/fetch_hash/struct.FetchHash.html) many ways. Here are the steps for one way to use it, followed by sample code.

* Create a `registry.txt` file containing a whitespace-delimited list of files
  and their hashes. (This is the same format as [Pooch](https://pypi.org/project/pooch/). See section [Registry Creation](#registry-creation) for tips on creating this file.)

* As shown below, create a global static
 [`FetchHash`](https://docs.rs/fetch-hash/latest/fetch_hash/struct.FetchHash.html#method.new)
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
use fetch_hash::{ctor, FetchHash, FetchHashError};
use std::path::{Path, PathBuf};

#[ctor]
static STATIC_FETCH_HASH: FetchHash = FetchHash::new(
    include_str!("../registry.txt"),
    "https://raw.githubusercontent.com/CarlKCarlK/fetch-hash/main/tests/data/",
    "BAR_APP_DATA_DIR", // env_key
    "com",              // qualifier
    "Foo Corp",         // organization
    "Bar App",          // application
);

/// Download a data file.
pub fn sample_file<P: AsRef<Path>>(path: P) -> Result<PathBuf, FetchHashError> {
    STATIC_FETCH_HASH.fetch_file(path)
}

```

You can now use your `sample_file` function to download your files as needed.

<a name="registry-creation"></a>Registry Creation
------------------------

You can create your `registry.txt` file many ways. Here are the steps for one way to create it, followed by sample code.

* Upload your data files to the Internet.
   - For example, `Fetch-Hash`
  puts its data files
  in `tests/data`, so they upload to [this GitHub folder](https://github.com/CarlKCarlK/fetch-hash/tree/main/tests/data). In GitHub, by looking at the [raw view of a data file](https://github.com/CarlKCarlK/fetch-hash/blob/main/tests/data/small.fam), we see the root URL for these files. In `cargo.toml`, we keep these data file out of our crate via `exclude = ["tests/data/*"]`
* As shown below, write code that
  - Creates a [`FetchHash`](https://docs.rs/fetch-hash/latest/fetch_hash/struct.FetchHash.html#method.new) instance without registry contents.
  - Lists the files in your data directory.
  - Calls the [`gen_registry_contents`](https://docs.rs/fetch-hash/latest/fetch_hash/struct.FetchHash.html#method.gen_registry_contents) method on your list of files. This method will download
    the files, compute their hashes, and create a string of file names and hashes.
* Print this string, then manually paste it into a file called `registry.txt`.

```rust
use fetch_hash::{FetchHash, dir_to_file_list};

let fetch_hash = FetchHash::new(
    "", // registry_contents ignored
    "https://raw.githubusercontent.com/CarlKCarlK/fetch-hash/main/tests/data/",
    "BAR_APP_DATA_DIR", // env_key
    "com",              // qualifier
    "Foo Corp",         // organization
    "Bar App",          // application
);
let file_list = dir_to_file_list("tests/data")?;
let registry_contents = fetch_hash.gen_registry_contents(file_list)?;
println!("{registry_contents}");

# use fetch_hash::FetchHashError; // '#' needed for doctest
# Ok::<(), FetchHashError>(())
```

Notes
-----

* Feature requests and contributions are welcome.
* Don't use our sample `sample_file`. Define your own `sample_file` that
  knows where to find *your* data files.
* The [`FetchHash`](https://docs.rs/fetch-hash/latest/fetch_hash/struct.FetchHash.html) instance need not be global and static. See [`FetchHash::new`](https://docs.rs/fetch-hash/latest/fetch_hash/struct.FetchHash.html#method.new) for an example of a non-global instance.
* Other [`methods on the FetchHash`](https://docs.rs/fetch-hash/latest/fetch_hash/struct.FetchHash.html#implementations) instance can fetch multiples files
and can give the path to the local data directory.
* You need not use a `registry.txt` file 
and [`FetchHash`](https://docs.rs/fetch-hash/latest/fetch_hash/struct.FetchHash.html) instance. You can instead use the stand-alone function [`fetch`](https://docs.rs/fetch-hash/latest/fetch_hash/fn.fetch.html) to retrieve a single file with known URL, hash, and local path.
* Other [stand-alone functions](https://docs.rs/fetch-hash/latest/fetch_hash/#functions) can download files, hash files, and create temporary 
directories.
* `Fetch-Hash` always does binary downloads to maintain consistant line endings across OSs.
* The [Bed-Reader](https://crates.io/crates/bed-reader) genomics crate
uses `Fetch-Hash`.
* Debugging this crate under Windows can cause a "Oops! The debug adapter has terminated abnormally" exception. This is some kind of [LLVM, Windows, NVIDIA(?) problem](https://github.com/vadimcn/vscode-lldb/issues/410) via ureq.
* This crate follows [Nine Rules for Elegant Rust Library APIs](https://towardsdatascience.com/nine-rules-for-elegant-rust-library-apis-9b986a465247) from *Towards Data Science*.


Project Links
-----

* [**Installation**](https://crates.io/crates/fetch-hash)
* [**Documentation**](https://docs.rs/fetch-hash/)
* [**Source code**](https://github.com/CarlKCarlK/fetch-hash)
* [**Discussion**](https://github.com/CarlKCarlK/fetch-hash/discussions/)
* [**Bug Reports and Feature Requests**](https://github.com/CarlKCarlK/fetch-hash/issues)
