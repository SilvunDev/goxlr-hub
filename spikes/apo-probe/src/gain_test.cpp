// Tests for the gain kernel. Exit code 0 means every check passed.
#include "gain.h"

#include <cstdio>

static int failures = 0;

static void check(bool ok, const char* what) {
    if (!ok) {
        std::printf("FAIL: %s\n", what);
        failures++;
    }
}

int main() {
    const float in[4] = {1.0f, -1.0f, 0.5f, 0.0f};
    float out[4] = {9.0f, 9.0f, 9.0f, 9.0f};
    apply_gain(out, in, 4, PROBE_GAIN);
    check(out[0] == 0.25f && out[1] == -0.25f && out[2] == 0.125f && out[3] == 0.0f,
          "scales every sample by the gain");

    float inplace[2] = {0.8f, -0.4f};
    apply_gain(inplace, inplace, 2, 0.5f);
    check(inplace[0] == 0.4f && inplace[1] == -0.2f, "works in place");

    float untouched[1] = {7.0f};
    apply_gain(untouched, in, 0, PROBE_GAIN);
    check(untouched[0] == 7.0f, "zero samples writes nothing");

    if (failures == 0) {
        std::printf("gain_test: ok\n");
    }
    return failures;
}
