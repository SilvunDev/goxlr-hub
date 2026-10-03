// Gain kernel of the probe effect. Kept free of Windows types so it can be
// tested on its own.
#pragma once

#include <cstddef>

// -12 dB: large enough to measure and to hear, and it cannot clip.
const float PROBE_GAIN = 0.25f;

// `out` may alias `in`: the audio engine hands in-place effects one buffer.
inline void apply_gain(float* out, const float* in, std::size_t samples, float gain) {
    for (std::size_t i = 0; i < samples; i++) {
        out[i] = in[i] * gain;
    }
}
