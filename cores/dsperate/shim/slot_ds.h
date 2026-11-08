#pragma once
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct slot_ds slot_ds;

#define SLOT_DS_ABI 1
#define SLOT_DS_W 256
#define SLOT_DS_H 384

uint32_t slot_ds_abi(void);
double slot_ds_fps(void);
double slot_ds_sample_rate(void);

slot_ds* slot_ds_open(const char* bios_dir);
int slot_ds_load(slot_ds* ds, const char* rom);
void slot_ds_reset(slot_ds* ds);
void slot_ds_run_frame(slot_ds* ds, uint16_t buttons, uint8_t touch_x, uint8_t touch_y, int touch_down, int lid_shut);
void slot_ds_skip_draw(slot_ds* ds, int skip);
const uint32_t* slot_ds_screens(slot_ds* ds);
size_t slot_ds_take_audio(slot_ds* ds, int16_t* dst, size_t max_frames);

size_t slot_ds_save_size(slot_ds* ds);
size_t slot_ds_save_read(slot_ds* ds, uint8_t* dst, size_t n);
int slot_ds_save_write(slot_ds* ds, const uint8_t* src, size_t n);
int slot_ds_save_dirty(slot_ds* ds);

size_t slot_ds_state_save(slot_ds* ds, const uint8_t** out);
int slot_ds_state_load(slot_ds* ds, const uint8_t* src, size_t n);

void slot_ds_close(slot_ds* ds);

#ifdef __cplusplus
}
#endif
