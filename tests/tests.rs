use cmake_package::{find_cmake, find_package, Error, Version};
use scopeguard::defer;
use serial_test::serial;

mod common;

// Note: since the tests manipulate the PATH environment variable in order to fake the presence
// of cmake, they must be executed serially, otherwise, since they are all executed within the
// same process, they would interfere with each other.

#[test]
#[serial]
fn test_find_cmake() {
    let cmake = find_cmake().expect("Failed to find cmake");
    assert!(cmake.path.exists());
    assert!(cmake.path.is_file());
    assert!(cmake.version >= Version::parse("3.19").unwrap());
}

#[test]
#[serial]
fn test_cmake_not_in_path() {
    let path = std::env::var("PATH").expect("Path not set?");
    defer! {
        std::env::set_var("PATH", path);
    }

    std::env::set_var("PATH", "");
    match find_cmake().expect_err("cmake should not be found when PATH is empty") {
        Error::CMakeNotFound => {}
        err => panic!("Unexpected error: expected CMakeNotFound, got {:?}", err),
    }
}

#[test]
#[serial]
fn test_cmake_version_too_old() {
    let _path_guard = common::use_cmake("cmake_old");

    match find_cmake().expect_err("cmake should be too old") {
        Error::UnsupportedCMakeVersion => {}
        err => panic!(
            "Unexpected error: expected UnsupportedCMakeVersion, got {:?}",
            err
        ),
    }
}

#[test]
#[serial]
fn test_missing_find_package() {
    let _tmpdir = common::set_outdir();

    match find_package("ThisPackageDefinitelyDoesNotExist")
        .verbose()
        .find()
        .expect_err("Found a package that possibly cannot exist")
    {
        Error::PackageNotFound => {}
        err => panic!("Unexpected error: expected PackageNotFound, got {:?}", err),
    }
}

#[test]
#[serial]
#[cfg(target_os = "linux")]
fn test_find_openssl() {
    let _tmpdir = common::set_outdir();

    let package = find_package("OpenSSL")
        .verbose()
        .find()
        .expect("Failed to find OpenSSL");
    assert_eq!(package.name, "OpenSSL");
    assert!(package.version.is_none()); // OpenSSL version detection simply doesn't work for some reason

    let target = package
        .target("OpenSSL::SSL")
        .expect("Failed to find OpenSSL::SSL target");
    assert_eq!(target.name, "OpenSSL::SSL");
    assert_eq!(target.include_directories, vec!["/usr/include"]);
    assert_eq!(target.link_libraries.len(), 2);
    // The actual path will vary depending on the system, and the library may itself may have a
    // soname, so we just check for the presence of the library name.
    assert!(target
        .link_libraries
        .iter()
        .any(|lib| lib.contains("libcrypto.so")));
    assert!(target
        .link_libraries
        .iter()
        .any(|lib| lib.contains("libssl.so")));
}

#[test]
#[serial]
#[cfg(target_os = "linux")]
#[ignore = "Requires Qt installed"]
fn test_find_qt() {
    let _tmpdir = common::set_outdir();

    let package = find_package("Qt6")
        .components(vec!["Core".into(), "Gui".into(), "Widgets".into()])
        .version("6.2")
        .verbose()
        .find()
        .expect("Failed to find Qt6");
    assert_eq!(package.name, "Qt6");
    assert!(package.version.unwrap() >= Version::parse("6.2").unwrap());
    assert_eq!(
        package.components,
        Some(vec!["Core".into(), "Gui".into(), "Widgets".into()])
    );

    let core = package
        .target("Qt6::Core")
        .expect("Failed to find Qt6::Core target");
    assert_eq!(core.name, "Qt6::Core");
    assert!(core.location.unwrap().contains("libQt6Core.so"));
    assert_eq!(
        core.include_directories,
        vec![
            "/usr/include/qt6".to_string(),
            "/usr/include/qt6/QtCore".to_string(),
            "/usr/lib/qt6/mkspecs/linux-g++".to_string()
        ]
    );

    let gui = package
        .target("Qt6::Gui")
        .expect("Failed to find Qt6::Gui target");
    assert_eq!(gui.name, "Qt6::Gui");
    assert!(gui.location.unwrap().contains("libQt6Gui.so"));
    assert_eq!(
        gui.compile_definitions,
        vec!["QT_CORE_LIB".to_string(), "QT_GUI_LIB".to_string()]
    );
    assert_eq!(
        gui.include_directories,
        vec![
            "/usr/include".to_string(),
            "/usr/include/qt6".to_string(),
            "/usr/include/qt6/QtCore".to_string(),
            "/usr/include/qt6/QtGui".to_string(),
            "/usr/lib/qt6/mkspecs/linux-g++".to_string()
        ]
    );
    assert!(gui.link_libraries.len() >= 4);
    assert!(gui.link_libraries.iter().find(|lib| lib.contains("libQt6Core.so")).is_some());
    assert!(gui.link_libraries.iter().find(|lib| lib.contains("libQt6Gui.so")).is_some());
    assert!(gui.link_libraries.iter().find(|lib| lib.contains("libOpenGL.so")).is_some());
    assert!(gui.link_libraries.iter().find(|lib| lib.contains("libGLX.so")).is_some());
}
