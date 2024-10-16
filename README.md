# cmake-package

A Rust crate to for [Cargo build scripts][cargo_build_script] to find [CMake packages][cmake_package] 
installed on the system and link against them. This is especially useful when your Rust project depends
on a system library that only provides a CMake package. This is essentially similar to the 
[`pkg-config`][crate_pkgconfig] crate, but for CMake.

Refer to the [documentation](https://docs.rs/cmake-package) for more information on usage.

## License

This project is licensed under the MIT license. See the [LICENSE](LICENSE) file for more information.

## Contribution

All contributions are welcome. Please open an issue or a pull request if you have any problem, suggestions or
improvement!

[cmake_package]: https://cmake.org/cmake/help/latest/manual/cmake-packages.7.html
[cargo_build_script]: https://doc.rust-lang.org/cargo/reference/build-scripts.html
[crate_pkgconfig]: https://docs.rs/pkg-config/0.3.31/pkg_config/
