cmake_minimum_required(VERSION 3.22)
project(terra_preview C CXX)
get_filename_component(FIRMWARE_DIR "${CMAKE_CURRENT_LIST_DIR}/../.." ABSOLUTE)
if(NOT DEFINED ENV{IDF_PATH})
    message(FATAL_ERROR "Activate ESP-IDF first (headers are used by app_state.h)")
endif()
set(LV_BUILD_CONF_PATH "${CMAKE_CURRENT_LIST_DIR}/lv_conf.h" CACHE PATH "" FORCE)
set(CONFIG_LV_BUILD_DEMOS OFF CACHE BOOL "" FORCE)
set(CONFIG_LV_BUILD_EXAMPLES OFF CACHE BOOL "" FORCE)
set(CONFIG_LV_USE_THORVG_INTERNAL OFF CACHE BOOL "" FORCE)
add_subdirectory("${FIRMWARE_DIR}/managed_components/lvgl__lvgl" lvgl)
add_executable(ui-preview preview.c
    "${FIRMWARE_DIR}/main/display/ui.c"
    "${FIRMWARE_DIR}/main/util/time_utils.c")
target_include_directories(ui-preview PRIVATE "${FIRMWARE_DIR}/main"
    "${FIRMWARE_DIR}/build/config"
    "$ENV{IDF_PATH}/components/esp_common/include")
target_link_libraries(ui-preview PRIVATE lvgl)
# Read the canonical panel size without importing ESP-only driver headers on host.
set_property(DIRECTORY APPEND PROPERTY CMAKE_CONFIGURE_DEPENDS "${FIRMWARE_DIR}/main/board_config.h")
file(READ "${FIRMWARE_DIR}/main/board_config.h" BOARD_CONFIG)
foreach(AXIS WIDTH HEIGHT)
    string(REGEX MATCH "#define BOARD_LCD_${AXIS}[ \t]+([0-9]+)" MATCHED "${BOARD_CONFIG}")
    if(NOT MATCHED)
        message(FATAL_ERROR "Missing numeric BOARD_LCD_${AXIS}")
    endif()
    target_compile_definitions(ui-preview PRIVATE PREVIEW_${AXIS}=${CMAKE_MATCH_1})
endforeach()
