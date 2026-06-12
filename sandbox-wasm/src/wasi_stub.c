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
// OCCT(getenv 等)が引きずる環境変数 import。環境は空として扱う。出力ポインタには 0 を書く。
int __imported_wasi_snapshot_preview1_environ_sizes_get(unsigned *count, unsigned *buf_size) {
	*count = 0; *buf_size = 0;
	return 0;
}
int __imported_wasi_snapshot_preview1_environ_get(int environ, int buf) {
	(void)environ; (void)buf;
	return 0;
}
// stdio 初期化の isatty 等が引く fd_fdstat_get。BADF を返して無効 fd 扱いにする。
int __imported_wasi_snapshot_preview1_fd_fdstat_get(int fd, int stat) {
	(void)fd; (void)stat;
	return 8; /* __WASI_ERRNO_BADF */
}
// fd_read。0 バイト読込(EOF)として返す。
int __imported_wasi_snapshot_preview1_fd_read(int fd, int iovs, int iovs_len, unsigned *nread) {
	(void)fd; (void)iovs; (void)iovs_len;
	*nread = 0;
	return 0;
}
// OCCT の Standard_ErrorHandler が参照する setjmp/longjmp。wasm では env import になる。
// シグナル経由の例外変換は使わず C++ 例外を使うので、setjmp は 0 を返すだけでよく
// (保護ブロックへ素通り)、longjmp は到達しない（万一来たら trap）。
int setjmp(void *env) { (void)env; return 0; }
void longjmp(void *env, int val) { (void)env; (void)val; __builtin_trap(); }
// 静的デストラクタ登録を抑止する。一回限りの実行なので終了時クリーンアップは不要で、
// __wasm_call_dtors 経由で OCCT 静的オブジェクトの dtor が不正な間接呼び出し
// (null function / signature mismatch) を起こすのを根本回避する。
int __cxa_atexit(void (*f)(void *), void *arg, void *dso) {
	(void)f; (void)arg; (void)dso;
	return 0;
}
