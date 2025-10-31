cmake_minimum_required(VERSION 3.20)

project(yarrp VERSION 0.0.1 LANGUAGES)

# download symcrypt
set(release_url https://github.com/microsoft/SymCrypt/releases/download/v103.8.0/symcrypt-windows-amd64-release-103.8.0-53be637d.zip)

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

# generate test cert
if(WIN32)
# Remove-Item Cert:\\CurrentUser\\My\\<thumbprint>
# get the test certificate
execute_process(COMMAND pwsh.exe -Command "Get-ChildItem Cert:\\CurrentUser\\My | Where-Object -Property FriendlyName -EQ -Value YARRP-Test | Select-Object -ExpandProperty Thumbprint -First 1"
  OUTPUT_VARIABLE TEST_CERT_HASH
)

if(TEST_CERT_HASH STREQUAL "")
  message(status "Generating TEST_CERT_HASH")
  execute_process(
    COMMAND pwsh.exe -Command "New-SelfSignedCertificate -Subject YARRP -DnsName $env:computername,localhost -FriendlyName YARRP-Test -KeyUsageProperty Sign -KeyUsage DigitalSignature -CertStoreLocation cert:\\CurrentUser\\My -HashAlgorithm SHA256 -Provider \"Microsoft Software Key Storage Provider\" -KeyExportPolicy Exportable"
    COMMAND_ERROR_IS_FATAL ANY
  )
  execute_process(COMMAND pwsh.exe -Command "Get-ChildItem Cert:\\CurrentUser\\My | Where-Object -Property FriendlyName -EQ -Value YARRP-Test | Select-Object -ExpandProperty Thumbprint -First 1"
    OUTPUT_VARIABLE TEST_CERT_HASH
    COMMAND_ERROR_IS_FATAL ANY
  )
endif()
message(STATUS "Using TEST_CERT_HASH ${TEST_CERT_HASH}")
endif(WIN32)