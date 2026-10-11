#pragma once
#include <stddef.h>
#include <stdint.h>

typedef enum { DISK_UNKNOWN, DISK_MBR, DISK_GPT, DISK_NTFS_SIGNATURE, DISK_INVALID } disk_layout_t;
typedef struct {
    uint8_t type;
    uint32_t start;
    uint32_t sectors;
} disk_partition_t;
typedef struct {
    disk_layout_t layout;
    size_t count;
    disk_partition_t partitions[4];
} disk_inspection_t;

/* Only recognizes metadata; does not mount or implement a filesystem. */
disk_inspection_t disk_inspect(const uint8_t *sector, size_t bytes, uint32_t sectors);
