use crate::version::{Version, VersionError};
use std::path::PathBuf;
use std::process::Command;
use serde::Deserialize;
use tempfile::tempdir_in;
use which::which;

const CMAKE_MIN_VERSION: &str = "3.19";

#[derive(Debug, Clone)]
pub struct CMakeProgram {
    path: PathBuf,
    version: Version,
}

fn script_path(script: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("cmake")
        .join(script)
}

#[derive(Debug)]
pub enum Error {
    CMakeNotFound,
    InternalError,
    IOError(std::io::Error),
    VersionError(VersionError),
    PackageNotFound,
}


#[derive(Clone, Debug, Deserialize)]
struct PackageResult {
    name: Option<String>,
    version: Option<String>,
}

pub fn find_cmake() -> Result<CMakeProgram, Error> {
    let path = which("cmake").or(Err(Error::CMakeNotFound))?;

    let output = Command::new(&path)
        .arg("-P")
        .arg(script_path("cmake_version.cmake"))
        .output()
        .or(Err(Error::InternalError))?;

    let version = String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_string()
        .try_into()
        .or(Err(Error::VersionError(VersionError::InvalidVersion)))?;

    if version < "3.15".try_into().map_err(Error::VersionError)? {
        return Err(Error::VersionError(VersionError::VersionTooOld(version)));
    }

    Ok(CMakeProgram { path, version })
}

use crate::{CMakePackage, CMakeTarget};

pub fn find_package(name: String, version: Option<Version>) -> Result<CMakePackage, Error> {
    // Find cmake or panic
    let cmake = find_cmake()?;

    // Prepare directory where we will generate our CMakeLists.txt
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap_or_else(|_| {
        panic!("OUT_DIR is not set, are you running the crate from build.rs?")
    }));

    // Make a unique directory inside
    let dir = tempdir_in(out_dir).or(Err(Error::InternalError))?;

    std::fs::copy(script_path("find_package.cmake"), dir.path().join("CMakeLists.txt")).map_err(Error::IOError)?;

    let package_json = dir.path().join("package.json");
    // Run the CMake script
    let mut command = Command::new(&cmake.path);
    command
        .current_dir(dir.path())
        .arg(".")
        .arg(format!("-DCMAKE_MIN_VERSION={CMAKE_MIN_VERSION}"))
        .arg(format!("-DPACKAGE={}", name));
        .arg(format!("-DOUTPUT_FILE={}", package_json.display()));
    if let Some(version) = version {
        command.arg(format!("-DVERSION={}", version));
    }
    command.output().map_err(Error::IOError)?;

    let reader = std::fs::File::open(package_json).map_err(Error::IOError)?;
    let package: PackageResult = serde_json::from_reader(reader).or(Err(Error::InternalError))?;

    let package_name = match package.name {
        Some(name) => name,
        None => return Err(Error::PackageNotFound),
    };

    let package_version = match package.version {
        Some(version) => Some(version.try_into().map_err(Error::VersionError)?),
        None => None,
    };

    // If the user requested a minimum version
    if let Some(version) = version {
        // And if we managed to get a version from the package
        if let Some(package_version) = package_version {
            if package_version < version {
                return Err(Error::VersionError(VersionError::VersionTooOld(package_version)));
            }
        }

        // It's not an error if the package did not provide a version.
    }

    Ok(CMakePackage::new(cmake, dir, package_name, package_version))
}

pub fn find_target(package: &CMakePackage, target: impl Into<String>) -> Option<CMakeTarget> {
    let target = target.into();

    // Run the CMake script
    let mut command = Command::new(&package.cmake.path);
    command
        .current_dir(package.working_directory.path())
        .arg(".")
        .arg(format!("-DCMAKE_MIN_VERSION={CMAKE_MIN_VERSION}"))
        .arg(format!("-DPACKAGE={}", package.name))
        .arg(format!("-DTARGET={}", target));
    if let Some(version) = package.version {
        command.arg(format!("-DVERSION={}", version));
    }
    command.output().ok()?;

    let reader = std::fs::File::open(package.working_directory.path().join(format!("target_{}.json", target))).ok()?;
    let target: CMakeTarget = serde_json::from_reader(reader).ok()?;

    Some(target)
}