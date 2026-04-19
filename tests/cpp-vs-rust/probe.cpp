// probe.cpp — thin stdin/stdout shim over upstream lensfun for A/B testing
// Protocol: TAB-separated commands, one per line. EOF → exit 0.
// See README.md for full protocol docs.

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>
#include <chrono>
#include "lensfun.h"

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

static std::string strip_quotes(const std::string &s) {
    if (s.size() >= 2 && s.front() == '"' && s.back() == '"')
        return s.substr(1, s.size() - 2);
    return s;
}

static std::vector<std::string> split_tabs(const std::string &line) {
    std::vector<std::string> parts;
    size_t start = 0;
    for (size_t i = 0; i <= line.size(); ++i) {
        if (i == line.size() || line[i] == '\t') {
            parts.push_back(line.substr(start, i - start));
            start = i + 1;
        }
    }
    return parts;
}

// Locate best-matching lens. Returns NULL and emits stderr message on failure.
static const lfLens *find_lens(const lfDatabase *db,
                                const char *maker, const char *model) {
    const lfLens **lenses = lf_db_find_lenses(db, NULL, maker, model, 0);
    if (!lenses || !lenses[0]) {
        fprintf(stderr, "ERROR: no lens match for maker=%s model=%s\n", maker, model);
        lf_free(lenses);
        return NULL;
    }
    // Results are sorted by score descending; take index 0.
    const lfLens *best = lenses[0];
    lf_free(lenses);
    return best;
}

// ---------------------------------------------------------------------------
// Command handlers
// ---------------------------------------------------------------------------

// distortion <maker> <model> <focal> <crop> <width> <height> <reverse> <x> <y>
static void cmd_distortion(const lfDatabase *db,
                            const std::vector<std::string> &f) {
    if (f.size() < 10) { fprintf(stderr, "ERROR: distortion: wrong field count\n"); return; }
    std::string maker = strip_quotes(f[1]);
    std::string model = strip_quotes(f[2]);
    float focal    = (float)atof(f[3].c_str());
    float crop     = (float)atof(f[4].c_str());
    int   width    = atoi(f[5].c_str());
    int   height   = atoi(f[6].c_str());
    bool  reverse  = f[7] != "0";
    float x        = (float)atof(f[8].c_str());
    float y        = (float)atof(f[9].c_str());

    const lfLens *lens = find_lens(db, maker.c_str(), model.c_str());
    if (!lens) { printf("nan\tnan\n"); return; }

    lfModifier *mod = lf_modifier_create(lens, focal, crop, width, height, LF_PF_F32, reverse);
    lf_modifier_enable_distortion_correction(mod);

    float coords[2];
    lf_modifier_apply_geometry_distortion(mod, x, y, 1, 1, coords);
    lf_modifier_destroy(mod);

    printf("%.10g\t%.10g\n", (double)coords[0], (double)coords[1]);
}

// tca <maker> <model> <focal> <crop> <width> <height> <reverse> <x> <y>
static void cmd_tca(const lfDatabase *db,
                    const std::vector<std::string> &f) {
    if (f.size() < 10) { fprintf(stderr, "ERROR: tca: wrong field count\n"); return; }
    std::string maker = strip_quotes(f[1]);
    std::string model = strip_quotes(f[2]);
    float focal    = (float)atof(f[3].c_str());
    float crop     = (float)atof(f[4].c_str());
    int   width    = atoi(f[5].c_str());
    int   height   = atoi(f[6].c_str());
    bool  reverse  = f[7] != "0";
    float x        = (float)atof(f[8].c_str());
    float y        = (float)atof(f[9].c_str());

    const lfLens *lens = find_lens(db, maker.c_str(), model.c_str());
    if (!lens) { printf("nan\tnan\tnan\tnan\n"); return; }

    lfModifier *mod = lf_modifier_create(lens, focal, crop, width, height, LF_PF_F32, reverse);
    lf_modifier_enable_tca_correction(mod);

    // ApplySubpixelDistortion returns 6 floats: Rx,Ry, Gx,Gy, Bx,By
    float coords[6];
    lf_modifier_apply_subpixel_distortion(mod, x, y, 1, 1, coords);
    lf_modifier_destroy(mod);

    // Output: x_red, y_red, x_blue, y_blue
    printf("%.10g\t%.10g\t%.10g\t%.10g\n",
           (double)coords[0], (double)coords[1],
           (double)coords[4], (double)coords[5]);
}

// vignetting <maker> <model> <focal> <aperture> <distance> <crop> <width> <height> <x> <y>
static void cmd_vignetting(const lfDatabase *db,
                            const std::vector<std::string> &f) {
    if (f.size() < 11) { fprintf(stderr, "ERROR: vignetting: wrong field count\n"); return; }
    std::string maker    = strip_quotes(f[1]);
    std::string model    = strip_quotes(f[2]);
    float focal          = (float)atof(f[3].c_str());
    float aperture       = (float)atof(f[4].c_str());
    float distance       = (float)atof(f[5].c_str());
    float crop           = (float)atof(f[6].c_str());
    int   width          = atoi(f[7].c_str());
    int   height         = atoi(f[8].c_str());
    float x              = (float)atof(f[9].c_str());
    float y              = (float)atof(f[10].c_str());

    const lfLens *lens = find_lens(db, maker.c_str(), model.c_str());
    if (!lens) { printf("nan\n"); return; }

    // reverse=false: correct vignetting in a real image
    lfModifier *mod = lf_modifier_create(lens, focal, crop, width, height, LF_PF_F32, false);
    lf_modifier_enable_vignetting_correction(mod, aperture, distance);

    // Apply colour modification to a single 1×1 F32 pixel of value 1.0.
    // The result is the vignetting gain at (x, y).
    float pixel = 1.0f;
    int comp_role = LF_CR_1(INTENSITY);
    lf_modifier_apply_color_modification(mod, &pixel, x, y, 1, 1, comp_role, sizeof(float));
    lf_modifier_destroy(mod);

    printf("%.10g\n", (double)pixel);
}

// ---------------------------------------------------------------------------
// bench command handlers
// ---------------------------------------------------------------------------

// bench distortion <maker> <model> <focal> <crop> <width> <height> <reverse> <iterations>
// → <elapsed_ns>
static void cmd_bench_distortion(const lfDatabase *db,
                                  const std::vector<std::string> &f) {
    if (f.size() < 9) { fprintf(stderr, "ERROR: bench distortion: wrong field count\n"); return; }
    std::string maker = strip_quotes(f[2]);
    std::string model = strip_quotes(f[3]);
    float focal    = (float)atof(f[4].c_str());
    float crop     = (float)atof(f[5].c_str());
    int   width    = atoi(f[6].c_str());
    int   height   = atoi(f[7].c_str());
    bool  reverse  = f[8] != "0";
    long  iters    = (f.size() >= 10) ? atol(f[9].c_str()) : 1000000;

    const lfLens *lens = find_lens(db, maker.c_str(), model.c_str());
    if (!lens) { printf("-1\n"); return; }

    lfModifier *mod = lf_modifier_create(lens, focal, crop, width, height, LF_PF_F32, reverse);
    lf_modifier_enable_distortion_correction(mod);

    float x = width * 0.3f;
    float y = height * 0.3f;
    float coords[2];

    // Warmup: ~10K iterations
    for (int i = 0; i < 10000; ++i) {
        lf_modifier_apply_geometry_distortion(mod, x, y, 1, 1, coords);
        asm volatile("" ::: "memory");
    }

    auto t0 = std::chrono::steady_clock::now();
    for (long i = 0; i < iters; ++i) {
        lf_modifier_apply_geometry_distortion(mod, x, y, 1, 1, coords);
        asm volatile("" ::: "memory");
    }
    auto t1 = std::chrono::steady_clock::now();

    lf_modifier_destroy(mod);

    long long elapsed_ns = std::chrono::duration_cast<std::chrono::nanoseconds>(t1 - t0).count();
    printf("%lld\n", elapsed_ns);
}

// bench tca <maker> <model> <focal> <crop> <width> <height> <reverse> <iterations>
// → <elapsed_ns>
static void cmd_bench_tca(const lfDatabase *db,
                           const std::vector<std::string> &f) {
    if (f.size() < 9) { fprintf(stderr, "ERROR: bench tca: wrong field count\n"); return; }
    std::string maker = strip_quotes(f[2]);
    std::string model = strip_quotes(f[3]);
    float focal    = (float)atof(f[4].c_str());
    float crop     = (float)atof(f[5].c_str());
    int   width    = atoi(f[6].c_str());
    int   height   = atoi(f[7].c_str());
    bool  reverse  = f[8] != "0";
    long  iters    = (f.size() >= 10) ? atol(f[9].c_str()) : 1000000;

    const lfLens *lens = find_lens(db, maker.c_str(), model.c_str());
    if (!lens) { printf("-1\n"); return; }

    lfModifier *mod = lf_modifier_create(lens, focal, crop, width, height, LF_PF_F32, reverse);
    lf_modifier_enable_tca_correction(mod);

    float x = width * 0.3f;
    float y = height * 0.3f;
    float coords[6];

    // Warmup
    for (int i = 0; i < 10000; ++i) {
        lf_modifier_apply_subpixel_distortion(mod, x, y, 1, 1, coords);
        asm volatile("" ::: "memory");
    }

    auto t0 = std::chrono::steady_clock::now();
    for (long i = 0; i < iters; ++i) {
        lf_modifier_apply_subpixel_distortion(mod, x, y, 1, 1, coords);
        asm volatile("" ::: "memory");
    }
    auto t1 = std::chrono::steady_clock::now();

    lf_modifier_destroy(mod);

    long long elapsed_ns = std::chrono::duration_cast<std::chrono::nanoseconds>(t1 - t0).count();
    printf("%lld\n", elapsed_ns);
}

// bench vignetting <maker> <model> <focal> <aperture> <distance> <crop> <width> <height> <iterations>
// → <elapsed_ns>
static void cmd_bench_vignetting(const lfDatabase *db,
                                  const std::vector<std::string> &f) {
    if (f.size() < 10) { fprintf(stderr, "ERROR: bench vignetting: wrong field count\n"); return; }
    std::string maker    = strip_quotes(f[2]);
    std::string model    = strip_quotes(f[3]);
    float focal          = (float)atof(f[4].c_str());
    float aperture       = (float)atof(f[5].c_str());
    float distance       = (float)atof(f[6].c_str());
    float crop           = (float)atof(f[7].c_str());
    int   width          = atoi(f[8].c_str());
    int   height         = atoi(f[9].c_str());
    long  iters          = (f.size() >= 11) ? atol(f[10].c_str()) : 100000;

    const lfLens *lens = find_lens(db, maker.c_str(), model.c_str());
    if (!lens) { printf("-1\n"); return; }

    lfModifier *mod = lf_modifier_create(lens, focal, crop, width, height, LF_PF_F32, false);
    lf_modifier_enable_vignetting_correction(mod, aperture, distance);

    float x = width * 0.3f;
    float y = height * 0.3f;
    int comp_role = LF_CR_1(INTENSITY);

    // Warmup
    for (int i = 0; i < 10000; ++i) {
        float pixel = 1.0f;
        lf_modifier_apply_color_modification(mod, &pixel, x, y, 1, 1, comp_role, sizeof(float));
        asm volatile("" ::: "memory");
    }

    auto t0 = std::chrono::steady_clock::now();
    for (long i = 0; i < iters; ++i) {
        float pixel = 1.0f;
        lf_modifier_apply_color_modification(mod, &pixel, x, y, 1, 1, comp_role, sizeof(float));
        asm volatile("" ::: "memory");
    }
    auto t1 = std::chrono::steady_clock::now();

    lf_modifier_destroy(mod);

    long long elapsed_ns = std::chrono::duration_cast<std::chrono::nanoseconds>(t1 - t0).count();
    printf("%lld\n", elapsed_ns);
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

int main() {
    const char *db_path = "/Users/veszelovszki/projects-git/vdavid/lensfun-rs/data/db";

    lfDatabase *db = lf_db_create();
    lfError err = lf_db_load_path(db, db_path);
    if (err != LF_NO_ERROR) {
        fprintf(stderr, "ERROR: failed to load database at %s (err=%d)\n", db_path, err);
        lf_db_destroy(db);
        return 1;
    }

    std::string line;
    char buf[4096];
    while (fgets(buf, sizeof(buf), stdin)) {
        // Strip trailing newline
        size_t len = strlen(buf);
        while (len > 0 && (buf[len-1] == '\n' || buf[len-1] == '\r'))
            buf[--len] = '\0';
        if (len == 0) continue;

        line.assign(buf);
        std::vector<std::string> fields = split_tabs(line);
        if (fields.empty()) continue;

        const std::string &cmd = fields[0];
        if (cmd == "distortion") {
            cmd_distortion(db, fields);
        } else if (cmd == "tca") {
            cmd_tca(db, fields);
        } else if (cmd == "vignetting") {
            cmd_vignetting(db, fields);
        } else if (cmd == "bench" && fields.size() >= 2) {
            const std::string &sub = fields[1];
            if (sub == "distortion") {
                cmd_bench_distortion(db, fields);
            } else if (sub == "tca") {
                cmd_bench_tca(db, fields);
            } else if (sub == "vignetting") {
                cmd_bench_vignetting(db, fields);
            } else {
                fprintf(stderr, "ERROR: unrecognized bench sub-command: %s\n", sub.c_str());
                lf_db_destroy(db);
                return 1;
            }
        } else {
            fprintf(stderr, "ERROR: unrecognized command: %s\n", cmd.c_str());
            lf_db_destroy(db);
            return 1;
        }
        fflush(stdout);
    }

    lf_db_destroy(db);
    return 0;
}
