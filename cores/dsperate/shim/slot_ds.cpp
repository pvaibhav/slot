#include "slot_ds.h"

#include "core/input/input_log.h"
#include "core/nds.h"
#include "core/state/state.h"
#include "blip_buf/blip_buf.h"
#if DSPERATE_JIT
#include "core/cpu/jit/jit.h"
#endif

#include <cstdio>
#include <cstring>
#include <string>
#include <sys/stat.h>
#include <vector>

constexpr int SLOT_DS_BLIP_SIZE = 8192;
constexpr double SLOT_DS_OUTPUT_HZ = 48000.0;

struct slot_ds {
  ds::NDS nds;
  std::string rom;
  std::vector<uint32_t> screens = std::vector<uint32_t>(SLOT_DS_W * SLOT_DS_H, 0xFF000000u);
  std::vector<ds::u8> state;
  bool jit = false;
  blip_t* left = blip_new(SLOT_DS_BLIP_SIZE);
  blip_t* right = blip_new(SLOT_DS_BLIP_SIZE);
  ds::s16 last[2] = {0, 0};
  ~slot_ds() {
    blip_delete(left);
    blip_delete(right);
  }
};

namespace {

bool exists(const std::string& path) {
  struct stat st;
  return stat(path.c_str(), &st) == 0 && S_ISREG(st.st_mode);
}

constexpr int SLOT_DS_CLOCKS = 512;
constexpr size_t SLOT_DS_CHUNK = 512;

void clear_audio(slot_ds* ds) {
  blip_clear(ds->left);
  blip_clear(ds->right);
  ds->last[0] = ds->last[1] = 0;
}

void feed_audio(slot_ds* ds) {
  const double clock = ds->nds.spu.output_rate_hz() * SLOT_DS_CLOCKS;
  blip_set_rates(ds->left, clock, SLOT_DS_OUTPUT_HZ);
  blip_set_rates(ds->right, clock, SLOT_DS_OUTPUT_HZ);
  ds::s16 raw[SLOT_DS_CHUNK * 2];
  size_t n;
  while ((n = ds->nds.spu.take(raw, SLOT_DS_CHUNK)) != 0) {
    if (blip_samples_avail(ds->left) > SLOT_DS_BLIP_SIZE - 2048) continue;
    unsigned t = 0;
    for (size_t i = 0; i < n; ++i) {
      t += SLOT_DS_CLOCKS;
      const int l = raw[i * 2], r = raw[i * 2 + 1];
      if (l != ds->last[0]) blip_add_delta(ds->left, t, l - ds->last[0]);
      if (r != ds->last[1]) blip_add_delta(ds->right, t, r - ds->last[1]);
      ds->last[0] = static_cast<ds::s16>(l);
      ds->last[1] = static_cast<ds::s16>(r);
    }
    blip_end_frame(ds->left, t);
    blip_end_frame(ds->right, t);
  }
}

bool boot(slot_ds* ds) {
  clear_audio(ds);
  ds->nds.reset();
  if (!ds->nds.load_rom(ds->rom)) return false;
  ds->nds.setup_direct_boot();
  ds->nds.io.start_rtc_clock();
#if DSPERATE_JIT
  if (!ds->jit) ds->jit = ds::jit::attach(ds->nds, true, true);
#endif
  return true;
}

void copy_screens(slot_ds* ds) {
  for (int s = 0; s < 2; ++s)
    std::memcpy(ds->screens.data() + s * SLOT_DS_W * 192, ds->nds.gpu.framebuffer(s), SLOT_DS_W * 192 * 4);
}

}

extern "C" {

uint32_t slot_ds_abi(void) { return SLOT_DS_ABI; }
double slot_ds_fps(void) { return 59.8261; }
double slot_ds_sample_rate(void) { return SLOT_DS_OUTPUT_HZ; }

slot_ds* slot_ds_open(const char* bios_dir) {
  slot_ds* ds = new slot_ds;
  std::string dir = bios_dir ? bios_dir : "";
  std::string b9 = dir + "/bios9.bin", b7 = dir + "/bios7.bin", fw = dir + "/firmware.bin";
  const bool real = !dir.empty() && exists(b9) && exists(b7) && exists(fw);
  std::string err;
  if (!ds->nds.load_bios(real ? b9 : "", real ? b7 : "", real ? fw : "", {}, &err)) {
    std::fprintf(stderr, "slot_ds: bios: %s\n", err.c_str());
    if (!real || !ds->nds.load_bios("", "", "", {}, &err)) {
      delete ds;
      return nullptr;
    }
  }
  return ds;
}

int slot_ds_load(slot_ds* ds, const char* rom) {
  ds->rom = rom ? rom : "";
  return boot(ds) ? 0 : 1;
}

void slot_ds_reset(slot_ds* ds) {
  std::vector<ds::u8> save;
  if (ds->nds.cart) save = ds->nds.cart->sram();
  if (boot(ds) && !save.empty()) ds->nds.cart->load_save(save.data(), save.size());
}

void slot_ds_run_frame(slot_ds* ds, uint16_t buttons, uint8_t touch_x, uint8_t touch_y, int touch_down, int lid_shut) {
  ds::input::Frame f;
  f.buttons = buttons;
  f.x = touch_x;
  f.y = touch_y;
  f.down = touch_down != 0;
  f.lid = lid_shut != 0;
  ds::input::apply(ds->nds, f);
  ds->nds.run_frame();
  copy_screens(ds);
}

void slot_ds_skip_draw(slot_ds* ds, int skip) { ds->nds.gpu.set_frame_skip(skip != 0); }

const uint32_t* slot_ds_screens(slot_ds* ds) { return ds->screens.data(); }

size_t slot_ds_take_audio(slot_ds* ds, int16_t* dst, size_t max_frames) {
  feed_audio(ds);
  const int want = static_cast<int>(max_frames < static_cast<size_t>(SLOT_DS_BLIP_SIZE) ? max_frames : SLOT_DS_BLIP_SIZE);
  const int got = blip_read_samples(ds->left, dst, want, 1);
  blip_read_samples(ds->right, dst + 1, got, 1);
  return static_cast<size_t>(got);
}

size_t slot_ds_save_size(slot_ds* ds) { return ds->nds.cart ? ds->nds.cart->sram().size() : 0; }

size_t slot_ds_save_read(slot_ds* ds, uint8_t* dst, size_t n) {
  if (!ds->nds.cart) return 0;
  const std::vector<ds::u8>& s = ds->nds.cart->sram();
  const size_t k = n < s.size() ? n : s.size();
  std::memcpy(dst, s.data(), k);
  ds->nds.cart->clear_sram_dirty();
  return k;
}

int slot_ds_save_write(slot_ds* ds, const uint8_t* src, size_t n) {
  if (!ds->nds.cart) return 1;
  ds->nds.cart->load_save(src, n);
  return 0;
}

int slot_ds_save_dirty(slot_ds* ds) { return ds->nds.cart && ds->nds.cart->sram_dirty() ? 1 : 0; }

size_t slot_ds_state_save(slot_ds* ds, const uint8_t** out) {
  ds::state::Writer w;
  std::string err;
  if (!ds->nds.save_state(w, err)) {
    std::fprintf(stderr, "slot_ds: save state: %s\n", err.c_str());
    *out = nullptr;
    return 0;
  }
  ds->state = std::move(w.data());
  *out = ds->state.data();
  return ds->state.size();
}

int slot_ds_state_load(slot_ds* ds, const uint8_t* src, size_t n) {
  std::vector<ds::u8> save;
  if (ds->nds.cart) save = ds->nds.cart->sram();
  ds::state::Reader r(src, n);
  std::string err;
  if (ds->nds.load_state(r, err)) {
    clear_audio(ds);
    copy_screens(ds);
    return 0;
  }
  std::fprintf(stderr, "slot_ds: load state: %s\n", err.c_str());
  if (boot(ds) && !save.empty()) ds->nds.cart->load_save(save.data(), save.size());
  return 1;
}

void slot_ds_close(slot_ds* ds) {
#if DSPERATE_JIT
  if (ds->jit) ds::jit::detach(ds->nds);
#endif
  delete ds;
}

}
