// GATE 0 part 1 — #220 GATE A' の再確認。
// in-wasm WasmFS に書いて読み戻し、C++ 例外(exnref)も飛ばす。--embed-file は使わない。
// 期待: import は wasi_snapshot_preview1 のみ (env=0)、bare wasmtime で動く。
#include <stdio.h>
#include <string.h>

static int throw_and_catch() {
    int caught = 0;
    for (int i = 0; i < 7; i++) {
        try { throw i; } catch (int) { caught++; }
    }
    return caught;
}

int main() {
    const char* paths[] = {"/x.txt", "/tmp/x.txt"};
    const char* payload = "INWASM-FS-OK";
    for (int i = 0; i < 2; i++) {
        FILE* w = fopen(paths[i], "w");
        if (!w) { fprintf(stderr, "fopen(w) failed: %s\n", paths[i]); return 1; }
        fwrite(payload, 1, strlen(payload), w);
        fclose(w);
        FILE* r = fopen(paths[i], "r");
        if (!r) { fprintf(stderr, "fopen(r) failed: %s\n", paths[i]); return 1; }
        char buf[64] = {0};
        size_t n = fread(buf, 1, sizeof(buf) - 1, r);
        fclose(r);
        buf[n] = 0;
        printf("READBACK[%s]=[%s]\n", paths[i], buf);
    }
    printf("EH=%d\n", throw_and_catch());
    return 0;
}
