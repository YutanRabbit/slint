# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

idf_component_register(
    SRCS "src/slint-esp.cpp"
    INCLUDE_DIRS "include"
    REQUIRES "esp_lcd" "esp_lcd_touch"
    PRIV_REQUIRES "esp_timer")

if (CONFIG_IDF_TARGET_ARCH_XTENSA)
    set(Rust_CARGO_TARGET "xtensa-${IDF_TARGET}-none-elf")
else(CONFIG_IDF_TARGET_ARCH_XTENSA)
    message(FATAL_ERROR "Architecture currently not supported")
endif(CONFIG_IDF_TARGET_ARCH_XTENSA)

set(SLINT_FEATURE_FREESTANDING ON)
set(SLINT_FEATURE_RENDERER_SOFTWARE ON)
set(SLINT_LIBRARY_CARGO_FLAGS "-Zbuild-std=core,alloc")
set(DEFAULT_SLINT_EMBED_RESOURCES "embed-for-software-renderer" CACHE STRING "")
set(CMAKE_BUILD_TYPE Release)
set(BUILD_SHARED_LIBS OFF)


if (SLINT_ESP_LOCAL_EXAMPLE)
    add_subdirectory(../.. slint_build)
else(SLINT_ESP_LOCAL_EXAMPLE)
    include(FetchContent)
    FetchContent_Declare(
        Slint
        GIT_REPOSITORY https://github.com/slint-ui/slint
        GIT_TAG v1.4.0
        SOURCE_SUBDIR api/cpp
    )
    FetchContent_MakeAvailable(Slint)
endif(SLINT_ESP_LOCAL_EXAMPLE)

target_link_libraries(${COMPONENT_LIB} PUBLIC Slint::Slint)
