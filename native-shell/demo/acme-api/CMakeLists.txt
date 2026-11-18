cmake_minimum_required(VERSION 3.24)
project(acme-api LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

add_library(greeter STATIC src/util/greeter.cpp)
target_include_directories(greeter PUBLIC include)

add_executable(acme src/main.cpp)
target_link_libraries(acme PRIVATE greeter)

add_executable(acme_tests tests/greeter_test.cpp)
target_link_libraries(acme_tests PRIVATE greeter)
