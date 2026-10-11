#include "disk_inspect.h"
#include <string.h>

static uint32_t le32(const uint8_t *p)
{
    return (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);
}

disk_inspection_t disk_inspect(const uint8_t *sector, size_t bytes, uint32_t sectors)
{
    disk_inspection_t result = {.layout = DISK_UNKNOWN};
    if (!sector || bytes < 512 || sectors == 0) {
        result.layout = DISK_INVALID;
        return result;
    }
    if (sector[510] != 0x55 || sector[511] != 0xaa) return result;
    if (memcmp(sector + 3, "NTFS    ", 8) == 0) {
        result.layout = DISK_NTFS_SIGNATURE;
        return result;
    }
    for (size_t i = 0; i < 4; ++i) {
        const uint8_t *entry = sector + 446 + i * 16;
        if (entry[4] == 0) continue;
        disk_partition_t part = {entry[4], le32(entry + 8), le32(entry + 12)};
        if ((entry[0] != 0 && entry[0] != 0x80) || part.start == 0 || part.sectors == 0
            || (uint64_t)part.start + part.sectors > sectors) {
            result.layout = DISK_INVALID;
            result.count = 0;
            return result;
        }
        if (part.type == 0xee) {
            result.layout = DISK_GPT;
            result.count = 0;
            return result;
        }
        result.partitions[result.count++] = part;
    }
    if (result.count) result.layout = DISK_MBR;
    return result;
}
