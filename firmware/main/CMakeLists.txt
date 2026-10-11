idf_component_register(
    SRCS "app_main.c" "app_state.c" "display/display.c" "display/ui.c"
         "util/time_utils.c" "input/input_logic.c" "input/input.c"
         "storage/disk_inspect.c" "storage/usb_probe.c" "diag/pin_scan.c"
    INCLUDE_DIRS "."
    PRIV_REQUIRES esp_driver_gpio esp_driver_spi esp_lcd esp_psram freertos esp_timer
)
target_compile_options(${COMPONENT_LIB} PRIVATE -Wall -Wextra -Werror)
