/* Minimal vendored config for SpeexDSP AEC (echo cancellation only).
 * Compiled with MSVC via the `cc` crate. We use the FLOATING_POINT path
 * (spx_word16_t = float) and the portable USE_SMALLFT FFT backend (no FFTW3). */
#ifndef SPEEXDSP_VENDOR_CONFIG_H
#define SPEEXDSP_VENDOR_CONFIG_H

#define FLOATING_POINT 1
#define USE_SMALLFT 1

#endif
