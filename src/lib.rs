//! A simple CMake package finder.
//! 
//! This crate is intended to be used in build.rs scripts to obtain information about
//! existing system CMake package, such as its include directories and link libraries
//! for individual CMake targets defined in the package.
//!
//! The crate runs the `cmake` command in the background to query the system for the
//! package and to extract the necessary information. CMake version 3.20 or higher is
//! required. The crate will panic if the `cmake` command is not found on the system
//! or if the version is too low.
//! 
//! The crate will search for the `cmake` command in the system `PATH` environment variable,
//! however it is possible to provide a custom path to the `cmake` command by setting the
//! `CMAKE_PACKAGE_CMAKE` environment variable.
//! 
//! If you want to make your dependency on CMake optional, you can use the `find_cmake()`
//! function to check that a suitable version of CMake is found without the crate panicking
//! and then decide on how to proceed yourself.
//! 
//! # Example
//! ```rust
//! use cmake_finder::find_package;
//! 
//! let package = find_package("OpenSSL").version("1.0").find();
//! let target = match package {
//!     None => panic!("OpenSSL>=1.0 not found"),
//!     Some(package) => {
//!         package.target("OpenSSL::SSL").unwrap()
//!     }
//! };
//! 
//! println!("Include directories: {:?}", target.include_directories());
//! target.link_libraries().iter().for_each(|lib| {
//!     println!("cargo:rustc-link-lib=dylib={}", lib.display());
//! });
//! ```
use std::path::PathBuf;

use cmake::CMakeProgram;
use serde::Deserialize;
use tempfile::TempDir;
use version::Version;

mod cmake;
mod version;

/// A CMake package.
#[derive(Debug)]
pub struct CMakePackage {
    cmake: CMakeProgram,
    working_directory: TempDir,
    name: String,
    version: Option<Version>,
}

impl CMakePackage {
    fn new(cmake: CMakeProgram, working_directory: TempDir, name: String, version: Option<Version>) -> Self {
        Self {
            cmake,
            working_directory,
            name,
            version,
        }
    }

    /// The name of the package.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The version of the package found on the system.
    pub fn version(&self) -> &Option<Version> {
        &self.version
    }

    pub fn target(&self, target: impl Into<String>) -> Option<CMakeTarget> {
        cmake::find_target(self, target)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CMakeTarget {
    pub name: String,
    pub include_directories: Vec<PathBuf>,
    pub link_libraries: Vec<PathBuf>,
}

impl CMakeTarget {
    fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            include_directories: Vec::new(),
            link_libraries: Vec::new(),
        }
    }

    pub fn include_directories(&self) -> &Vec<PathBuf> {
        &self.include_directories
    }

    pub fn link_libraries(&self) -> &Vec<PathBuf> {
        &self.link_libraries
    }
}


#[derive(Debug, Clone)]
pub struct FindPackageBuilder {
    name: String,
    version: Option<Version>,
}

impl FindPackageBuilder {
    fn new(name: String) -> Self {
        Self {
            name,
            version: None,
        }
    }

    pub fn version(self, version: impl TryInto<Version>) -> Self {
        Self {
            version: Some(version.try_into().unwrap_or_else(|_| panic!("Invalid version specified!"))),
            ..self
        }
    }

    pub fn find(self) -> Option<CMakePackage> {
        cmake::find_package(self.name, self.version)
    }
}

pub fn find_package(name: impl Into<String>) -> FindPackageBuilder {
    FindPackageBuilder::new(name.into())
}


#[cfg(test)]
mod testing {
    use super::*;

    #[test]
    fn test_find_package() {
        let package = find_package("foo").find();
        assert_eq!(package, None);
    }

    #[test]
    fn test_find_package_with_version() {
        let package = find_package("foo").version("1.0").find();
        assert_eq!(package, None);
    }
}
