# Enrobage Audio Unit v2 (.component) du CLAP « make it drum » via clap-wrapper.
# macOS seulement ; le CLAP universel doit déjà être construit par
# `cargo xtask bundle-universal make-it-drum --release`.
#
#   cmake -S au -B au/build -DCMAKE_BUILD_TYPE=Release
#   cmake --build au/build --parallel
#   → au/build/make it drum.component (le CLAP est embarqué dans Contents/PlugIns/)
#
# Variables :
#   MID_CLAP     chemin du bundle .clap à embarquer (défaut : target/bundled/make it drum.clap)
#   MID_VERSION  version du bundle (défaut : lue dans Cargo.toml)
cmake_minimum_required(VERSION 3.21)

if(NOT APPLE)
  message(FATAL_ERROR "L'Audio Unit ne se construit que sur macOS")
endif()

set(CMAKE_OSX_ARCHITECTURES "arm64;x86_64" CACHE STRING "Architectures (binaire universel)")
set(CMAKE_OSX_DEPLOYMENT_TARGET "10.13" CACHE STRING "macOS minimum")

# Version unique : celle du workspace Cargo.
if(NOT DEFINED MID_VERSION)
  file(STRINGS "${CMAKE_CURRENT_SOURCE_DIR}/../Cargo.toml" _mid_version_line REGEX "^version = \"")
  string(REGEX REPLACE "^version = \"([^\"]+)\".*$" "\\1" MID_VERSION "${_mid_version_line}")
endif()

project(make-it-drum-au VERSION ${MID_VERSION} LANGUAGES C CXX OBJC OBJCXX)

if(NOT DEFINED MID_CLAP)
  set(MID_CLAP "${CMAKE_CURRENT_SOURCE_DIR}/../target/bundled/make it drum.clap")
endif()
if(NOT IS_DIRECTORY "${MID_CLAP}")
  message(FATAL_ERROR "CLAP introuvable : ${MID_CLAP} — lancer d'abord cargo xtask bundle-universal make-it-drum --release")
endif()

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# SDK CLAP et AudioUnitSDK récupérés par clap-wrapper (CPM) à la configuration.
set(CLAP_WRAPPER_DOWNLOAD_DEPENDENCIES ON CACHE BOOL "Télécharger les SDK nécessaires")

include(FetchContent)
FetchContent_Declare(clap-wrapper
  GIT_REPOSITORY https://github.com/free-audio/clap-wrapper.git
  GIT_TAG 806bba0a5f792e3966b99c1561ab9952d3316d7c # main, 20 octobre 2026
)
FetchContent_MakeAvailable(clap-wrapper)

add_library(make_it_drum_au MODULE)
set_target_properties(make_it_drum_au PROPERTIES LIBRARY_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}")

# Codes AU : fabricant « SaGz » (Saiya Geez), sous-type « MkDr », type aumu (instrument).
target_add_auv2_wrapper(
  TARGET make_it_drum_au
  OUTPUT_NAME "make it drum"
  BUNDLE_IDENTIFIER "io.github.yrbane.make-it-drum"
  BUNDLE_VERSION "${PROJECT_VERSION}"
  MANUFACTURER_NAME "Masao Saiya Geez"
  MANUFACTURER_CODE "SaGz"
  SUBTYPE_CODE "MkDr"
  INSTRUMENT_TYPE "aumu"
  MACOS_EMBEDDED_CLAP_LOCATION "${MID_CLAP}"
)
