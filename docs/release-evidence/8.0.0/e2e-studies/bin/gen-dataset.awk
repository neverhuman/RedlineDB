# gen-dataset.awk — deterministic E2E training dataset for the WQ-9 studies harness.
#
# Emits a CSV to stdout: 6 numeric features + 1 categorical + a binary `target`
# with a strong PLANTED linear signal (so a real model trains to a non-trivial score).
# Fully deterministic: a Park-Miller MINSTD LCG with a fixed seed — no system RNG, so
# the byte output (and its sha256) is stable across hosts/runs.
#
# Precision note: MINSTD is `seed = (16807 * seed) % 2147483647`. The largest product is
# 16807 * 2147483646 ~= 3.6e13, well under 2^53, so awk's double arithmetic is EXACT.
#
# Usage: awk -f gen-dataset.awk            # 240 rows, seed 20260714 (defaults)
#        awk -v n=240 -v seed=20260714 -f gen-dataset.awk
BEGIN {
    if (n == 0)    n = 240
    if (seed == 0) seed = 20260714
    M = 2147483647               # 2^31 - 1 (MINSTD modulus)
    A = 16807

    # Park-Miller requires seed in [1, M-1]; normalise defensively.
    seed = seed % M
    if (seed <= 0) seed += (M - 1)

    ncat = split("alpha,beta,gamma", cats, ",")
    print "f1,f2,f3,f4,f5,f6,cat,target"

    pos = 0
    for (i = 0; i < n; i++) {
        f1 = rnd() * 10.0                 # [0,10)
        f2 = rnd() * 10.0                 # [0,10)  (noise feature, no signal)
        f3 = rnd() * 5.0                  # [0,5)
        f4 = rnd() * 2.0 - 1.0            # [-1,1)  (noise feature)
        f5 = rnd() * 100.0                # [0,100)
        f6 = rnd()                        # [0,1)   (noise feature)
        ci = int(rnd() * ncat) + 1
        cat = cats[ci]
        cateff = (cat == "alpha" ? 1.5 : (cat == "beta" ? -0.5 : 0.0))

        noise = (rnd() - 0.5) * 2.0       # ~U(-1,1), sd ~0.58 (small vs signal sd ~2.6)
        score = 0.8 * f1 - 0.6 * f3 + 0.03 * f5 + cateff + noise
        target = (score > 4.30) ? 1 : 0   # threshold ~= mean(score) => roughly balanced
        pos += target

        printf "%.4f,%.4f,%.4f,%.4f,%.4f,%.4f,%s,%d\n", f1, f2, f3, f4, f5, f6, cat, target
    }
    # class balance to stderr (diagnostic only; does not affect the CSV bytes)
    printf "gen-dataset: n=%d positives=%d (%.1f%%) seed=%d\n", n, pos, 100.0*pos/n, seed > "/dev/stderr"
}

function rnd() {
    seed = (A * seed) % M
    return seed / M
}
