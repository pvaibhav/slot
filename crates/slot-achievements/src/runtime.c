/* Small C ABI around the unmodified rcheevos runtime. Only the RA worker calls it. */
#include "rc_runtime.h"
#include <stdint.h>
#include <stddef.h>

typedef struct {
    const uint8_t *ram;
    const size_t *valid;
    uint32_t *earned;
    size_t capacity, count;
    int invalid;
} slot_frame;

/* The runtime's event/validation callbacks have no userdata argument. */
static _Thread_local slot_frame *current;

static int valid_address(uint32_t address) {
    size_t start;
    unsigned region;
    if (address < 0x8000) { start = 0; region = 0; }
    else if (address < 0x48000) { start = 0x8000; region = 1; }
    else { start = 0x48000; region = 2; }
    return address - start < current->valid[region];
}

static uint32_t peek(uint32_t address, uint32_t bytes, void *unused) {
    uint32_t value = 0, i;
    (void)unused;
    for (i = 0; i < bytes; ++i) {
        if (address > UINT32_MAX - i || !valid_address(address + i)) {
            current->invalid = 1;
            return 0;
        }
        value |= ((uint32_t)current->ram[address + i]) << (8 * i);
    }
    return value;
}

static void event(const rc_runtime_event_t *ev) {
    if (ev->type == RC_RUNTIME_EVENT_ACHIEVEMENT_TRIGGERED && current->count < current->capacity)
        current->earned[current->count++] = ev->id;
}

size_t slot_ra_frame(rc_runtime_t *runtime, const uint8_t *ram, const size_t *valid,
                     uint32_t *earned, size_t capacity) {
    slot_frame frame = { ram, valid, earned, capacity, 0, 0 };
    current = &frame;
    rc_runtime_validate_addresses(runtime, event, valid_address);
    rc_runtime_do_frame(runtime, event, peek, NULL, NULL);
    current = NULL;
    /* An invalid indirect read cannot count as an observed zero or satisfy a trigger. */
    if (frame.invalid) { rc_runtime_reset(runtime); return 0; }
    return frame.count;
}
