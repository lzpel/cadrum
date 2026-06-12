// wasm32-unknown-unknown には WASI ランタイムが無いが、libc++ の <iostream> が
// 生成する std::ios_base::Init 静的初期化子が stdio を参照し、その先で
// wasi_snapshot_preview1 への import (__imported_wasi_snapshot_preview1_*) が残る。
// 正常系では一切 stdout/stderr に書かないので、その実 import シンボルを no-op で
// 定義して import を消す。シグネチャは WASI ABI（i32/i64）に一致させる。
int __imported_wasi_snapshot_preview1_fd_write(int fd, int iovs, int iovs_len, int nwritten) {
	(void)fd; (void)iovs; (void)iovs_len; (void)nwritten;
	return 0;
}
int __imported_wasi_snapshot_preview1_fd_seek(int fd, long long offset, int whence, int newoffset) {
	(void)fd; (void)offset; (void)whence; (void)newoffset;
	return 0;
}
int __imported_wasi_snapshot_preview1_fd_close(int fd) {
	(void)fd;
	return 0;
}
// libc++abi の terminate / abort 経路が引きずる proc_exit。正常系では呼ばれない。
void __imported_wasi_snapshot_preview1_proc_exit(int code) {
	(void)code;
}
// 起動時の preopen 走査 (__wasilibc_populate_preopens) が引きずる。fd_prestat_get は
// エラーが返るまで fd を増やしながら呼ばれるので、BADF(8) を返して即座に走査を終わらせる。
int __imported_wasi_snapshot_preview1_fd_prestat_get(int fd, int buf) {
	(void)fd; (void)buf;
	return 8; /* __WASI_ERRNO_BADF */
}
int __imported_wasi_snapshot_preview1_fd_prestat_dir_name(int fd, int path, int path_len) {
	(void)fd; (void)path; (void)path_len;
	return 8; /* __WASI_ERRNO_BADF */
}
