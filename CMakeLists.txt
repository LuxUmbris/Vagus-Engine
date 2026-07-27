cmake_minimum_required(VERSION 3.16)
project(VagusEngine LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

include(GNUInstallDirs)

add_library(vagus_engine STATIC
    src/render.cpp
    src/window.cpp
    src/physics.cpp
)

target_compile_features(vagus_engine PUBLIC cxx_std_17)

target_include_directories(vagus_engine
    PUBLIC
        ${CMAKE_CURRENT_SOURCE_DIR}/include
)

find_package(OpenGL REQUIRED)
target_link_libraries(vagus_engine PUBLIC OpenGL::GL)

find_package(SDL2 QUIET)
if(TARGET SDL2::SDL2)
    target_link_libraries(vagus_engine PUBLIC SDL2::SDL2)
else()
    message(STATUS "SDL2 was not found via CMake; the build will rely on the system headers if available")
endif()

install(TARGETS vagus_engine
    ARCHIVE DESTINATION ${CMAKE_INSTALL_LIBDIR}
    LIBRARY DESTINATION ${CMAKE_INSTALL_LIBDIR}
    RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR}
)

install(DIRECTORY include/ DESTINATION ${CMAKE_INSTALL_INCLUDEDIR})
