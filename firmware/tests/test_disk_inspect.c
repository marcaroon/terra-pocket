#include "storage/disk_inspect.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>

static void put32(uint8_t *p, uint32_t v)
{
    for (unsigned i = 0; i < 4; ++i) p[i] = (uint8_t)(v >> (8 * i));
}
int main(void)
{
    uint8_t sector[4096] = {0};
    assert(disk_inspect(NULL, 512, 100).layout == DISK_INVALID);
    assert(disk_inspect(sector, 511, 100).layout == DISK_INVALID);
    assert(disk_inspect(sector, sizeof(sector), 0).layout == DISK_INVALID);
    assert(disk_inspect(sector, 512, 100).layout == DISK_UNKNOWN);
    sector[510] = 0x55; sector[511] = 0xaa;
    assert(disk_inspect(sector, 512, 100).layout == DISK_UNKNOWN);
    memcpy(sector + 3, "NTFS    ", 8);
    assert(disk_inspect(sector, 4096, 100).layout == DISK_NTFS_SIGNATURE);
    memset(sector + 3, 0, 8);
    uint8_t *part = sector + 446;
    part[4] = 7; put32(part + 8, 1); put32(part + 12, 99);
    disk_inspection_t result = disk_inspect(sector, 512, 100);
    assert(result.layout == DISK_MBR && result.count == 1);
    assert(result.partitions[0].start == 1 && result.partitions[0].sectors == 99);
    // Type 07 alone is not evidence of NTFS; only the volume signature is reported.
    part[4] = 0xee;
    assert(disk_inspect(sector, 512, 100).layout == DISK_GPT);
    part[4] = 7; part[0] = 1;
    assert(disk_inspect(sector, 512, 100).layout == DISK_INVALID);
    part[0] = 0x80; put32(part + 8, 99); put32(part + 12, 2);
    assert(disk_inspect(sector, 512, 100).layout == DISK_INVALID);
    put32(part + 8, UINT32_MAX - 2); put32(part + 12, 10);
    assert(disk_inspect(sector, 512, UINT32_MAX).layout == DISK_INVALID);
    put32(part + 8, 0); put32(part + 12, 99);
    assert(disk_inspect(sector, 512, 100).layout == DISK_INVALID);
    put32(part + 8, 1); put32(part + 12, 0);
    assert(disk_inspect(sector, 512, 100).layout == DISK_INVALID);
    for (unsigned i = 0; i < 4; ++i) {
        uint8_t *entry = sector + 446 + 16 * i;
        entry[4] = 7; put32(entry + 8, i * 10 + 1); put32(entry + 12, 10);
    }
    result = disk_inspect(sector, 512, 100);
    assert(result.count == 4 && result.partitions[3].start == 31);
    puts("PASS disk metadata: truncated, NTFS signature, MBR, GPT marker, bounds and overflow");
    return 0;
}
