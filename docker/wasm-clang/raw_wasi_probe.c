// GATE 1a: emscripten-standalone WASMFS の wasm から raw __wasi_* で wasmtime の
// preopen(--dir) を読めるか最小検証する。WASMFS は preopen を「見ない」が、それは
// WASMFS の open() 経路の話。WASI import を直接叩けば preopen fd を触れるはず、という仮説。
// 成立すれば bridge（preopen→MEMFS コピー）が作れる。
#include <stdio.h>
#include <string.h>
typedef unsigned int   u32;
typedef unsigned short u16;
typedef unsigned char  u8;
typedef unsigned long long u64;
#define WI(n) __attribute__((import_module("wasi_snapshot_preview1"), import_name(n)))

WI("fd_prestat_get")      int wasi_fd_prestat_get(int fd, void* prestat);
WI("fd_prestat_dir_name") int wasi_fd_prestat_dir_name(int fd, char* path, u32 path_len);
WI("path_open")           int wasi_path_open(int dirfd, u32 dirflags, const char* path, u32 path_len,
                                             u16 oflags, u64 rights_base, u64 rights_inh, u16 fdflags, int* opened);
WI("fd_read")             int wasi_fd_read(int fd, const void* iovs, u32 iovs_len, u32* nread);
WI("fd_close")            int wasi_fd_close(int fd);

typedef struct { void* buf; u32 len; } iov;
// prestat: u8 tag(0=dir) + pad + u32 name_len
typedef struct { u8 tag; u8 pad[3]; u32 name_len; } prestat_t;

#define R_FD_READ   (1ull<<1)
#define R_FD_SEEK   (1ull<<2)
#define R_FD_TELL   (1ull<<5)
#define R_FD_FSTAT  (1ull<<21)
#define R_PATH_OPEN (1ull<<13)
#define R_PFSTAT    (1ull<<18)
#define RD_RIGHTS (R_FD_READ|R_FD_SEEK|R_FD_TELL|R_FD_FSTAT|R_PATH_OPEN|R_PFSTAT)

int main(void) {
    // 1) preopen を列挙
    for (int fd = 3; fd < 64; fd++) {
        prestat_t pre;
        int rc = wasi_fd_prestat_get(fd, &pre);
        if (rc != 0) { fprintf(stderr, "fd %d: prestat rc=%d (stop)\n", fd, rc); break; }
        char name[1024] = {0};
        if (pre.name_len < sizeof name) { wasi_fd_prestat_dir_name(fd, name, pre.name_len); name[pre.name_len] = 0; }
        fprintf(stderr, "preopen fd=%d tag=%u name='%s'\n", fd, pre.tag, name);

        // 2) この preopen 直下の "probe.txt" を raw path_open で開いて読む
        int file = -1;
        rc = wasi_path_open(fd, 1 /*symlink_follow*/, "probe.txt", 9, 0 /*oflags*/, RD_RIGHTS, RD_RIGHTS, 0, &file);
        fprintf(stderr, "  path_open(probe.txt) rc=%d fd=%d\n", rc, file);
        if (rc == 0) {
            char buf[256] = {0};
            iov v = { buf, sizeof buf - 1 };
            u32 n = 0;
            int rr = wasi_fd_read(file, &v, 1, &n);
            buf[n] = 0;
            fprintf(stderr, "  READ rc=%d n=%u content=[%s]\n", rr, n, buf);
            wasi_fd_close(file);
        }
    }
    return 0;
}
