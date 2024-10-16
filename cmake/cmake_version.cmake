# SPDX-FileCopyrightText: 2024 Daniel Vrátil <dvratil@kde.org>
#
# SPDX-License-Identifier: MIT

# Print the current version of CMake to stderr - this is easier than parsing
# the output for `cmake --version`
message("${CMAKE_VERSION}")