// SPDX-FileCopyrightText: 2024 Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: MIT

use crate::version::{Version, VersionError};
use crate::{CMakePackage, CMakeTarget};

use itertools::Itertools;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::{tempdir_in, TempDir};
use which::which;

/// The minimum version of CMake required by this crate.
pub const CMAKE_MIN_VERSION: &str = "3.19";

/// A structure representing the CMake program found on the system.
#[derive(Debug, Clone)]
pub struct CMakeProgram {
    /// Path to the `cmake` executable to be used.
    pub path: PathBuf,
    /// Version of the `cmake` detected. Must be at least [`CMAKE_MIN_VERSION`].
    pub version: Version,
}

fn script_path(script: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("cmake")
        .join(script)
}

/// Errors tha can occur while working with CMake.
#[derive(Debug)]
pub enum Error {
    /// The `cmake` executable was not found in system `PATH` environment variable.
    CMakeNotFound,
    /// The available CMake version is too old (see [`CMAKE_MIN_VERSION`]).
    UnsupportedCMakeVersion,
    /// An internal error in the library.
    Internal,
    /// An I/O error while executing `cmake`
    IO(std::io::Error),
    /// An version-related error (e.g. the found package version is too old)
    Version(VersionError),
    /// The requested package was not found by CMake.
    PackageNotFound,
}

#[derive(Clone, Debug, Deserialize)]
struct PackageResult {
    name: Option<String>,
    version: Option<String>,
    components: Option<Vec<String>>,
}

/// Find the CMake program on the system and check version compatibility.
///
/// Tries to find the `cmake` executable in all paths listed in the `PATH` environment variable.
/// If found, it also checks that the version of CMake is at least [`CMAKE_MIN_VERSION`].
///
/// Returns [`CMakeProgram`] on success and [`Error::CMakeNotFound`] when the `cmake` executable
/// is not found or [`Error::UnspportedCMakeVersion`] when the version is too low.
pub fn find_cmake() -> Result<CMakeProgram, Error> {
    let path = which("cmake").or(Err(Error::CMakeNotFound))?;

    let output = Command::new(&path)
        .arg("-P")
        .arg(script_path("cmake_version.cmake"))
        .output()
        .or(Err(Error::Internal))?;

    let version = String::from_utf8_lossy(&output.stderr)
        .trim()
        .to_string()
        .try_into()
        .or(Err(Error::UnsupportedCMakeVersion))?;

    if version
        < CMAKE_MIN_VERSION
            .try_into()
            .map_err(|_| Error::UnsupportedCMakeVersion)?
    {
        return Err(Error::UnsupportedCMakeVersion);
    }

    Ok(CMakeProgram { path, version })
}

fn get_temporary_working_directory() -> Result<TempDir, Error> {
    #[cfg(test)]
    let out_dir = std::env::temp_dir();
    #[cfg(not(test))]
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_else(|_| {
        panic!("OUT_DIR is not set, are you running the crate from build.rs?")
    }));

    // Make a unique directory inside
    tempdir_in(out_dir).or(Err(Error::Internal))
}

fn setup_cmake_project(working_directory: &Path) -> Result<(), Error> {
    std::fs::copy(
        script_path("find_package.cmake"),
        working_directory.join("CMakeLists.txt"),
    )
    .map_err(Error::IO)?;
    Ok(())
}

/// Performs the actual `find_package()` operation with CMake
pub(crate) fn find_package(
    name: String,
    version: Option<Version>,
    components: Option<Vec<String>>,
) -> Result<CMakePackage, Error> {
    // Find cmake or panic
    let cmake = find_cmake()?;

    let working_directory = get_temporary_working_directory()?;

    setup_cmake_project(working_directory.path())?;

    let output_file = working_directory.path().join("package.json");
    // Run the CMake - see the find_package.cmake script for docs
    let mut command = Command::new(&cmake.path);
    command
        .current_dir(working_directory.path())
        .arg(".")
        .arg(format!("-DCMAKE_MIN_VERSION={CMAKE_MIN_VERSION}"))
        .arg(format!("-DPACKAGE={}", name))
        .arg(format!("-DOUTPUT_FILE={}", output_file.display()));
    if let Some(version) = version {
        command.arg(format!("-DVERSION={}", version));
    }
    if let Some(components) = components {
        command.arg(format!("-DCOMPONENTS={}", components.join(";")));
    }
    command.output().map_err(Error::IO)?;

    // Read from the generated JSON file
    let reader = std::fs::File::open(output_file).map_err(Error::IO)?;
    let package: PackageResult = serde_json::from_reader(reader).or(Err(Error::Internal))?;

    let package_name = match package.name {
        Some(name) => name,
        None => return Err(Error::PackageNotFound),
    };

    let package_version = match package.version {
        Some(version) => Some(version.try_into().map_err(Error::Version)?),
        None => None, // Missing version is not an error
    };

    if let Some(version) = version {
        if let Some(package_version) = package_version {
            if package_version < version {
                return Err(Error::Version(VersionError::VersionTooOld(package_version)));
            }
        }

        // It's not an error if the package did not provide a version.
    }

    Ok(CMakePackage::new(
        cmake,
        working_directory,
        package_name,
        package_version,
        package.components,
    ))
}

#[derive(Debug, Clone, Deserialize)]
enum PropertyValue {
    String(String),
    Target(Target),
}

impl From<PropertyValue> for Vec<String> {
    fn from(value: PropertyValue) -> Self {
        match value {
            PropertyValue::String(value) => vec![value],
            PropertyValue::Target(target) => match target.location {
                Some(location) => vec![location],
                None => vec![],
            }
            .into_iter()
            .chain(
                target
                    .interface_link_libraries
                    .into_iter()
                    .flat_map(Into::<Vec<String>>::into),
            )
            .collect(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct Target {
    name: String,
    location: Option<String>,
    location_release: Option<String>,
    location_debug: Option<String>,
    location_relwithdebinfo: Option<String>,
    location_minsizerel: Option<String>,
    interface_compile_definitions: Vec<String>,
    interface_compile_options: Vec<String>,
    interface_include_directories: Vec<String>,
    interface_link_directories: Vec<String>,
    interface_link_libraries: Vec<PropertyValue>,
    interface_link_options: Vec<String>,
}

/// Collects values from `property` of the current target and from `property` of
/// all targets linked in `interface_link_libraries` recursively.
///
/// This basically implements the CMake logic as described in the documentation
/// of e.g. [`INTERFACE_COMPILE_DEFINITIONS`][INTERFACE_COMPILE_DEFINITIONS] for
/// the target property:
///
/// > When target dependencies are specified using [`target_link_libraries()`][target_link_libraries],
/// > CMake will read this property from all target dependencies to determine the build properties of
/// > the consumer.
///
/// This function preserves the order of the values as they are found in the targets, the value of the
/// immediate `target` value is first, followed by all transitive properties of each linked target.
///
/// [INTERFACE_COMPILE_DEFINITIONS]: https://cmake.org/cmake/help/latest/prop_tgt/INTERFACE_COMPILE_DEFINITIONS.html
/// [target_link_libraries]: https://cmake.org/cmake/help/latest/command/target_link_libraries.html
fn collect_from_targets<'a>(
    target: &'a Target,
    property: impl Fn(&Target) -> Vec<String> + 'a + Copy,
) -> Vec<String> {
    property(target)
        .into_iter()
        .chain(
            target
                .interface_link_libraries
                .iter()
                .filter_map(|value| match value {
                    PropertyValue::String(_) => None,
                    PropertyValue::Target(target) => Some(target),
                })
                .flat_map(|target| collect_from_targets(target, property)),
        )
        .collect()
}

/// Equivalent to `collect_from_target`, but it sorts and deduplicates the properties - use with
/// care, as the order of the properties might be important (e.g. for compile options).
fn collect_from_targets_unique<'a>(
    target: &'a Target,
    property: impl Fn(&Target) -> Vec<String> + 'a + Copy,
) -> Vec<String> {
    collect_from_targets(target, property)
        .into_iter()
        .sorted()
        .dedup()
        .collect()
}

impl From<Target> for CMakeTarget {
    fn from(target: Target) -> Self {
        Self {
            compile_definitions: collect_from_targets_unique(&target, |target| {
                target.interface_compile_definitions.clone()
            }),
            compile_options: collect_from_targets(&target, |target| {
                target.interface_compile_options.clone()
            }),
            include_directories: collect_from_targets_unique(&target, |target| {
                target.interface_include_directories.clone()
            }),
            link_directories: collect_from_targets_unique(&target, |target| {
                target.interface_link_directories.clone()
            }),
            link_options: collect_from_targets(&target, |target| {
                target.interface_link_options.clone()
            }),
            link_libraries: target
                .location
                .as_ref()
                .map_or(vec![], |location| vec![location.clone()])
                .into_iter()
                .chain(
                    target
                        .interface_link_libraries
                        .into_iter()
                        .flat_map(Into::<Vec<String>>::into),
                )
                .sorted() // FIXME: should we really do this for libraries? Linking order might be important...
                .dedup()
                .collect(),
            name: target.name,
            location: target.location,
        }
    }
}

/// Finds the specified target in the CMake package and extracts its properties.
/// Returns `None` if the target was not found.
pub(crate) fn find_target(package: &CMakePackage, target: impl Into<String>) -> Option<CMakeTarget> {
    let target = target.into();

    // Run the CMake script
    let output_file = package
        .working_directory
        .path()
        .join(format!("target_{}.json", target));
    let mut command = Command::new(&package.cmake.path);
    command
        .current_dir(package.working_directory.path())
        .arg(".")
        .arg(format!("-DCMAKE_MIN_VERSION={CMAKE_MIN_VERSION}"))
        .arg(format!("-DPACKAGE={}", package.name))
        .arg(format!("-DTARGET={}", target))
        .arg(format!(
            "-DOUTPUT_FILE={}",
            output_file.display()
        ));
    if let Some(version) = package.version {
        command.arg(format!("-DVERSION={}", version));
    }
    if let Some(components) = &package.components {
        command.arg(format!("-DCOMPONENTS={}", components.join(";")));
    }
    command.output().ok()?;

    // Read from the generated JSON file
    let reader = std::fs::File::open(output_file).ok()?;
    let target: Target = serde_json::from_reader(reader).ok()?;

    Some(target.into())
}


#[cfg(test)]
mod testing {
    use super::*;

    #[test]
    fn from_target() {
        let target = Target {
            name: "my_target".to_string(),
            location: Some("/path/to/target.so".to_string()),
            location_release: Some("/path/to/target/release".to_string()),
            location_debug: Some("/path/to/target/debug".to_string()),
            location_relwithdebinfo: Some("/path/to/target/relwithdebinfo".to_string()),
            location_minsizerel: Some("/path/to/target/minsizerel".to_string()),
            interface_compile_definitions: vec!["DEFINE1".to_string(), "DEFINE2".to_string()],
            interface_compile_options: vec!["-O2".to_string(), "-Wall".to_string()],
            interface_include_directories: vec!["/path/to/include".to_string()],
            interface_link_directories: vec!["/path/to/lib".to_string()],
            interface_link_options: vec!["-L/path/to/lib".to_string()],
            interface_link_libraries: vec![
                PropertyValue::String("library1".to_string()),
                PropertyValue::String("library2".to_string()),
                PropertyValue::Target(Target {
                    name: "dependency".to_string(),
                    location: Some("/path/to/dependency.so".to_string()),
                    location_release: Some("/path/to/dependency/release".to_string()),
                    location_debug: Some("/path/to/dependency/debug".to_string()),
                    location_relwithdebinfo: Some("/path/to/dependency/relwithdebinfo".to_string()),
                    location_minsizerel: Some("/path/to/dependency/minsizerel".to_string()),
                    interface_compile_definitions: vec!["DEFINE3".to_string()],
                    interface_compile_options: vec!["-O3".to_string()],
                    interface_include_directories: vec!["/path/to/dependency/include".to_string()],
                    interface_link_directories: vec!["/path/to/dependency/lib".to_string()],
                    interface_link_options: vec!["-L/path/to/dependency/lib".to_string()],
                    interface_link_libraries: vec![PropertyValue::String(
                        "dependency_library".to_string(),
                    )],
                }),
            ],
        };

        let cmake_target: CMakeTarget = target.into();

        assert_eq!(cmake_target.name, "my_target");
        assert_eq!(cmake_target.location, Some("/path/to/target.so".into()));
        assert_eq!(
            cmake_target.location_release,
            Some("/path/to/target/release".into())
        );
        assert_eq!(
            cmake_target.location_debug,
            Some("/path/to/target/debug".into())
        );
        assert_eq!(
            cmake_target.location_relwithdebinfo,
            Some("/path/to/target/relwithdebinfo".into())
        );
        assert_eq!(
            cmake_target.location_minsizerel,
            Some("/path/to/target/minsizerel".into())
        );
        assert_eq!(
            cmake_target.compile_definitions,
            vec!["DEFINE1", "DEFINE2", "DEFINE3"]
        );
        assert_eq!(
            cmake_target.compile_options,
            vec!["-O2", "-Wall", "-O3"]
        );
        assert_eq!(
            cmake_target.include_directories,
            vec!["/path/to/dependency/include", "/path/to/include"]
        );
        assert_eq!(
            cmake_target.link_directories,
            vec!["/path/to/dependency/lib", "/path/to/lib"]
        );
        assert_eq!(
            cmake_target.link_options,
            vec!["-L/path/to/lib", "-L/path/to/dependency/lib"]
        );
        assert_eq!(
            cmake_target.link_libraries,
            vec![
                "/path/to/dependency.so",
                "/path/to/target.so",
                "dependency_library",
                "library1",
                "library2",
            ]
        );
    }
}
