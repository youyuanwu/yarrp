cmake_minimum_required(VERSION 3.20)

project(yarrp VERSION 0.0.1 LANGUAGES)

# download symcrypt
set(release_url https://github.com/microsoft/SymCrypt/releases/download/v103.4.2/symcrypt-windows-amd64-release-103.4.2-171f697.zip)

message(STATUS "fetching symcrypt_release " ${release_url})
include(FetchContent)
FetchContent_Declare(
  symcrypt_release
  DOWNLOAD_EXTRACT_TIMESTAMP true
  URL ${release_url}
)

FetchContent_GetProperties(symcrypt_release)
if(NOT symcrypt_release_POPULATED)
  FetchContent_Populate(symcrypt_release)
endif()

# copy symcrypt to rust dir
file(
  COPY ${symcrypt_release_SOURCE_DIR}/dll/symcrypt.dll
  DESTINATION ${CMAKE_CURRENT_SOURCE_DIR}/target/debug/deps
)