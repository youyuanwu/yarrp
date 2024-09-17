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

message(STATUS "fetching protoc")
include(FetchContent)
if(WIN32)
  FetchContent_Declare(
    protoc
    DOWNLOAD_NO_EXTRACT FALSE
    DOWNLOAD_EXTRACT_TIMESTAMP TRUE
    URL https://github.com/protocolbuffers/protobuf/releases/download/v24.1/protoc-24.1-win64.zip
    URL_HASH SHA256=DAD1EFED75CB772A0C9E5D4646006BF87E1E051191473A2A30C9DB66EE223CFD
  )
else(WIN32)
  FetchContent_Declare(
    protoc
    DOWNLOAD_NO_EXTRACT FALSE
    DOWNLOAD_EXTRACT_TIMESTAMP TRUE
    URL https://github.com/protocolbuffers/protobuf/releases/download/v26.0/protoc-26.0-linux-x86_64.zip
  )
endif(WIN32)
FetchContent_GetProperties(protoc)
if(NOT protoc_POPULATED)
  FetchContent_Populate(protoc)
endif()

# generate test cert
if(WIN32)
# get the test certificate
execute_process(COMMAND pwsh.exe -Command "Get-ChildItem Cert:\\CurrentUser\\My | Where-Object -Property FriendlyName -EQ -Value MsQuic-Test | Select-Object -ExpandProperty Thumbprint -First 1"
  OUTPUT_VARIABLE TEST_CERT_HASH
)

if(TEST_CERT_HASH STREQUAL "")
  message(Status "Generating TEST_CERT_HASH")
  execute_process(
    COMMAND pwsh.exe -Command "New-SelfSignedCertificate -DnsName $env:computername,localhost -FriendlyName MsQuic-Test -KeyUsageProperty Sign -KeyUsage DigitalSignature -CertStoreLocation cert:\\CurrentUser\\My -HashAlgorithm SHA256 -Provider \"Microsoft Software Key Storage Provider\" -KeyExportPolicy Exportable"
    COMMAND_ERROR_IS_FATAL ANY
  )
  execute_process(COMMAND pwsh.exe -Command "Get-ChildItem Cert:\\CurrentUser\\My | Where-Object -Property FriendlyName -EQ -Value MsQuic-Test | Select-Object -ExpandProperty Thumbprint -First 1"
    OUTPUT_VARIABLE TEST_CERT_HASH
    COMMAND_ERROR_IS_FATAL ANY
  )
endif()
message(STATUS "Using TEST_CERT_HASH ${TEST_CERT_HASH}")
endif(WIN32)