cmake_minimum_required(VERSION ${CMAKE_MIN_VERSION})
# We need at least the "C" language enabled, otherwise FindThreads will fail
project(cmake-package LANGUAGES C CXX)

function(find_package_wrapper)
    cmake_parse_arguments(FP "" "PACKAGE;VERSION;COMPONENTS;OUTPUT_FILE" "" ${ARGN})
    if (NOT FP_OUTPUT_FILE)
        message(FATAL_ERROR "OUTPUT_FILE is not set")
    endif()

    # Don't specify the version here, even if FP_VERSION is set - we want to find the package
    # even if the version is too old in order to be able to return the found version back to
    # the Rust code.
    find_package(${FP_PACKAGE} QUIET COMPONENTS ${FP_COMPONENTS})
    # Package found?
    if (${FP_PACKAGE}_FOUND)
        # Write its name into the JSON
        string(JSON json SET "{ }" "name" "\"${FP_PACKAGE}\"")
        # If we also found a version, write its version
        if (DEFINED ${FP_PACKAGE}_VERSION)
            string(JSON json SET ${json} "version" "\"${${FP_PACKAGE}_VERSION}\"")
        endif()

        file(WRITE ${FP_OUTPUT_FILE} ${json})
    else()
        # If not found, just output an empty JSON object, the rust code will interpret it as not found
        file(WRITE ${FP_OUTPUT_FILE} "{ }")
    endif()
endfunction()


function(resolve_target_prop)
    cmake_parse_arguments(ARG "" "TARGET;PROPERTY;OUT_VAR" "" ${ARGN})
    # Read the property value
    get_target_property(prop_values ${ARG_TARGET} ${ARG_PROPERTY})
    # Check each value
    message(STATUS "${ARG_TARGET}: ${ARG_PROPERTY} = ${prop_values}")
    set(result)
    foreach(value ${prop_values})
        # If the value is actually another imported target, then recursive into it and obtain
        # all properties of the target. Don't recurse into ourselves.
        if (TARGET ${value} AND NOT ("${ARG_TARGET}" STREQUAL "${value}"))
            set(var)
            resolve_deps_recursively(TARGET ${value} OUTPUT_JSON var)
            list(APPEND result ${var})
        elseif(value AND NOT ("${value}" MATCHES "^\\$\\<.*")) # Ignore generator expressions
            # Otherwise just append the value to output the list
            list(APPEND result ${value})
        endif()
    endforeach()
    set(${ARG_OUT_VAR} ${result} PARENT_SCOPE)
endfunction()

function(resolve_deps_recursively)
    cmake_parse_arguments(ARG "" "TARGET;OUTPUT_JSON" "" ${ARGN})
    set(props
        NAME
        LOCATION
        INTERFACE_COMPILE_DEFINITIONS
        INTERFACE_COMPILE_OPTIONS
        INTERFACE_INCLUDE_DIRECTORIES
        INTERFACE_LINK_DIRECTORIES
        INTERFACE_LINK_LIBRARIES
        INTERFACE_LINK_OPTIONS
    )
    set(cfg_props
        LOCATION
    )
    set(cfg_types
        Release
        RelWithDebInfo
        MinSizeRel
        Debug
    )
    foreach(cfg_prop ${cfg_props})
        foreach(config ${cfg_types})
            list(APPEND props "${cfg_prop}_${config}")
        endforeach()
    endforeach()

    set(json "{}")
    foreach(prop ${props})
        set(value)
        resolve_target_prop(TARGET ${ARG_TARGET} PROPERTY ${prop} OUT_VAR value)
        if (value)
            list_to_json(json ${json} ${prop} value)
        endif()
    endforeach()
    set(${ARG_OUTPUT_JSON} ${json} PARENT_SCOPE)

endfunction()

function(list_to_json json_var json member list_var)
    set(i 0)
    string(JSON json SET ${json} "${member}" "[]")
    foreach(elem ${${list_var}})
        # Super-simple check if "elem" contains a JSON object, otherwise assume string
        if (elem MATCHES "^{.*")
            string(JSON json SET ${json} "${member}" "${i}" "${elem}")
        else()
            string(JSON json SET ${json} "${member}" "${i}" "\"${elem}\"")
        endif()
        math(EXPR i "${i} + 1")
    endforeach()

    set(${json_var} ${json} PARENT_SCOPE)
endfunction()

function (find_package_target)
    cmake_parse_arguments(FP "" "PACKAGE;COMPONENTS;VERSION;TARGET;OUTPUT_FILE" "" ${ARGN})

    # It's safe to require the version here, we already found the package before and established
    # the version is recent enough.
    find_package(${FP_PACKAGE} ${FP_VERSION} COMPONENTS ${FP_COMPONENTS})
    if (${FP_PACKAGE}_FOUND)
        resolve_deps_recursively(
            TARGET ${FP_TARGET}
            OUTPUT_JSON json
        )
        file(WRITE ${FP_OUTPUT_FILE} ${json})
    else()
        # We found the package before, how come we did not find it this time?!
        message(FATAL_ERROR "Package ${FP_PACKAGE} not found")
    endif()
endfunction()

if (NOT DEFINED PACKAGE)
    message(FATAL_ERROR "PACKAGE is not set")
endif()

if (NOT DEFINED OUTPUT_FILE)
    message(FATAL_ERROR "OUTPUT_FILE is not set")
endif()

if (NOT DEFINED TARGET)
    find_package_wrapper(
        PACKAGE ${PACKAGE}
        COMPONENTS ${COMPONENTS}
        VERSION ${VERSION}
        OUTPUT_FILE ${OUTPUT_FILE}
    )
else()
    find_package_target(
        PACKAGE ${PACKAGE}
        COMPONENTS ${COMPONENTS}
        VERSION ${VERSION}
        TARGET ${TARGET}
        OUTPUT_FILE ${OUTPUT_FILE}
    )
endif()
